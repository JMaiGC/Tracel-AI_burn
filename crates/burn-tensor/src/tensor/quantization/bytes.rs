use core::any::TypeId;

use crate::{Bytes, Element};
use alloc::vec::Vec;

use super::{
    QParams, QuantInputType, QuantLevel, QuantMode, QuantScheme, QuantizationStrategy,
    SymmetricQuantization,
};

/// Quantized data bytes representation.
///
/// # Notes
/// 1) The quantized values are packed into 32-bit unsigned integers. For example, int8
///    quantized values pack 4 grouped values into a single `u32`. When unpacking these values,
///    we make sure to retrieve only the meaningful values (and ignore the alignment padding).
/// 2) Quantization parameters are appended to the tensor data.
///    As such, the last bytes always correspond to the scale parameter.
///    If the quantization scheme includes an offset (zero-point) parameter, it is next to last.
pub struct QuantizedBytes {
    /// The quantized values and quantization parameters represented as bytes.
    pub bytes: Bytes,
    /// The quantization scheme.
    pub scheme: QuantScheme,
    /// The number of quantized elements.
    pub num_elements: usize,
}

impl QuantizedBytes {
    /// Creates a new quantized bytes representation.
    pub fn new<E: Element>(value: Vec<E>, strategy: QuantizationStrategy) -> Self {
        let mut bytes: Bytes;
        let num_elements = value.len();
        let scheme = strategy.scheme();

        match strategy {
            QuantizationStrategy::PerTensorSymmetricInt8(quant) => {
                if TypeId::of::<E>() == TypeId::of::<i8>() {
                    // Re-interpret `Vec<E>` as `Vec<i8>` with `Vec::from_raw_parts`
                    let i8s: Vec<i8> = bytemuck::allocation::cast_vec(value);
                    bytes = Bytes::from_elems(i8s);
                } else {
                    panic!("Invalid quantized type");
                }
                let scale_bytes = bytemuck::bytes_of(&quant.scale);
                bytes.extend_from_byte_slice_aligned(scale_bytes, align_of::<f32>());
            }
            QuantizationStrategy::PerBlockSymmetricInt8(quant, _block_size) => {
                if TypeId::of::<E>() == TypeId::of::<i8>() {
                    // Re-interpret `Vec<E>` as `Vec<i8>` with `Vec::from_raw_parts`
                    let i8s: Vec<i8> = bytemuck::allocation::cast_vec(value);
                    bytes = Bytes::from_elems(i8s);
                } else {
                    panic!("Invalid quantized type");
                }

                let mut scale_bytes = Vec::with_capacity(quant.len() * size_of::<f32>());
                for q in quant {
                    scale_bytes.extend_from_slice(bytemuck::bytes_of(&q.scale));
                }
                bytes.extend_from_byte_slice_aligned(scale_bytes.as_slice(), align_of::<f32>());
            }
        }

        Self {
            bytes,
            scheme,
            num_elements,
        }
    }

    /// Returns the int8 quantized values with the quantization parameters.
    pub fn into_vec_i8(self) -> (Vec<i8>, QParams<Vec<f32>>) {
        let (values, (qparams, num_params)) = self.split_values_off();

        // Quantization parameters are added at the end of the tensor data.
        // As such, the last bytes always correspond to the scale parameter(s).
        // For example, per-block quantization can have multiple parameters for a single tensor:
        // [scale, scale, scale, ...]
        let scale_size = core::mem::size_of::<f32>(); // scale is stored as f32
        let qparams_bytes: &[u8] = bytemuck::cast_slice(&qparams);
        let total_bytes = qparams_bytes.len();

        let scales_size = scale_size * num_params;

        let scales = bytemuck::cast_slice(&qparams_bytes[total_bytes - scales_size..]).to_vec();

        (values, QParams { scales })
    }

    /// Splits the quantized values of the tensor from the quantization parameters.
    ///
    /// Returns the packed values and a newly allocated vector containing the quantization parameters.
    fn split_values_off(self) -> (Vec<i8>, (Vec<u32>, usize)) {
        let mut values = self.bytes.try_into_vec::<i8>().unwrap();

        let num_params = match self.scheme.level {
            QuantLevel::Tensor => 1,
            QuantLevel::Block(block_size) => self.num_elements / block_size,
        };

        let scale_size = num_params * size_of::<f32>();
        let values_end = values.len() - scale_size;

        let qparams = values.split_off(values_end);

        let qparams = if qparams.as_ptr() as usize % 4 == 0 {
            let mut qparams = core::mem::ManuallyDrop::new(qparams);
            unsafe {
                Vec::<u32>::from_raw_parts(
                    qparams.as_mut_ptr() as _,
                    qparams.len() / 4,
                    qparams.capacity() / 4,
                )
            }
        } else {
            #[cfg(target_endian = "little")]
            {
                // SAFETY: quantized bytes representation is created from packed u32 values in little endian
                bytemuck::cast_vec(qparams)
            }
            #[cfg(target_endian = "big")]
            {
                crate::quantization::pack_i8s_to_u32s(bytemuck::cast_vec(qparams))
            }
        };

        (values, (qparams, num_params))
    }

    /// Dequantizes the data according to its quantization scheme.
    pub fn dequantize(self) -> (Vec<f32>, QParams<Vec<f32>>) {
        match self.scheme {
            QuantScheme {
                level: QuantLevel::Tensor,
                mode: QuantMode::Symmetric,
                q_type: QuantInputType::QInt8,
                ..
            } => {
                let (values, qparams) = self.into_vec_i8();
                let strategy = QuantizationStrategy::PerTensorSymmetricInt8(
                    SymmetricQuantization::init(qparams.scales[0]),
                );
                (strategy.dequantize(&values), qparams)
            }
            QuantScheme {
                level: QuantLevel::Block(block_size),
                mode: QuantMode::Symmetric,
                q_type: QuantInputType::QInt8,
                ..
            } => {
                let (values, qparams) = self.into_vec_i8();
                assert_eq!(values.len() / qparams.scales.len(), block_size);
                let strategy = QuantizationStrategy::PerBlockSymmetricInt8(
                    qparams
                        .scales
                        .iter()
                        .map(|&s| SymmetricQuantization::init(s))
                        .collect(),
                    block_size,
                );
                (strategy.dequantize(&values), qparams)
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use alloc::vec;

    #[test]
    fn should_pack_unpack_quantization_parameters_per_tensor_symmetric() {
        // Quantized [[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]]
        let scale = 0.03937008;
        let values = vec![0i8, 25, 51, 76, 102, 127];

        let q_bytes = QuantizedBytes::new(
            values.clone(),
            QuantizationStrategy::PerTensorSymmetricInt8(SymmetricQuantization::init(scale)),
        );

        let (q_values, qparams) = q_bytes.into_vec_i8();

        assert_eq!(qparams.scales, vec![scale]);

        assert_eq!(q_values, values);
    }
}
