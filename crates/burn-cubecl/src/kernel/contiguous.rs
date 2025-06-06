use crate::{CubeRuntime, execute_with_dtype, tensor::CubeTensor};

/// Make a jit tensor contiguous.
pub fn into_contiguous<R: CubeRuntime>(tensor: CubeTensor<R>) -> CubeTensor<R> {
    if tensor.is_contiguous() {
        return tensor;
    }

    execute_with_dtype!(tensor.dtype, E, {
        let output =
            cubecl::std::tensor::into_contiguous::<R, E>(&tensor.client, &tensor.as_handle_ref());

        CubeTensor::new(
            tensor.client,
            output.handle,
            output.shape.into(),
            tensor.device,
            output.strides,
            tensor.dtype,
        )
    })
}

/// Make a jit tensor contiguous with an aligned last stride.
pub fn into_contiguous_aligned<R: CubeRuntime>(tensor: CubeTensor<R>) -> CubeTensor<R> {
    if tensor.is_contiguous() {
        return tensor;
    }

    execute_with_dtype!(tensor.dtype, E, {
        let output = cubecl::std::tensor::into_contiguous_pitched::<R, E>(
            &tensor.client,
            &tensor.as_handle_ref(),
        );

        CubeTensor::new(
            tensor.client,
            output.handle,
            output.shape.into(),
            tensor.device,
            output.strides,
            tensor.dtype,
        )
    })
}
