use crate::{CubeElement, CubeRuntime, tensor::CubeTensor};
use cubecl::std::tensor::index_offset_with_layout;
use cubecl::{calculate_cube_count_elemwise, prelude::*, tensor_vectorization_factor};
use std::any::TypeId;

#[cube(launch)]
pub(crate) fn cast_element<I: CubePrimitive, O: CubePrimitive>(
    input: &Tensor<Line<I>>,
    output: &mut Tensor<Line<O>>,
    #[comptime] rank: Option<u32>,
) {
    let offset_output = ABSOLUTE_POS;

    if offset_output >= output.len() {
        terminate!();
    }

    let offset_input = index_offset_with_layout::<I, O>(
        input,
        output,
        offset_output,
        0,
        rank.unwrap_or_else(|| output.rank()),
        rank.is_some(),
    );

    output[offset_output] = Line::cast_from(input[offset_input]);
}

/// Cast a tensor to the given element type.
///
/// Note: When input element is semantically a boolean, prefer bool_cast function.
pub fn cast<R: CubeRuntime, EI: CubeElement, EO: CubeElement>(
    input: CubeTensor<R>,
) -> CubeTensor<R> {
    if TypeId::of::<EI>() == TypeId::of::<EO>() {
        return CubeTensor::new_contiguous(
            input.client,
            input.device,
            input.shape,
            input.handle,
            input.dtype,
        );
    }

    // Vectorization is only enabled when the last dimension is contiguous.
    let rank = input.shape.num_dims();
    let vectorization_factor =
        tensor_vectorization_factor(&[4, 2], &input.shape.dims, &input.strides, rank - 1);

    let num_elems: usize = input.shape.num_elements();

    let cube_dim = CubeDim::default();
    let cube_count =
        calculate_cube_count_elemwise(num_elems / vectorization_factor as usize, cube_dim);
    let client = input.client.clone();
    let handle = client.empty(num_elems * core::mem::size_of::<EO>());
    let output = CubeTensor::new_contiguous(
        client.clone(),
        input.device.clone(),
        input.shape.clone(),
        handle,
        EO::dtype(),
    );

    cast_element::launch::<EI, EO, R>(
        &client,
        cube_count,
        cube_dim,
        input.as_tensor_arg::<EI>(vectorization_factor),
        output.as_tensor_arg::<EO>(vectorization_factor),
        Some(rank as u32),
    );

    output
}
