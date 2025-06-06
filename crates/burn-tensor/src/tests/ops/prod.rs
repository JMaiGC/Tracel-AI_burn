#[burn_tensor_testgen::testgen(prod)]
mod tests {
    use super::*;
    use burn_tensor::{Tensor, TensorData};

    #[test]
    fn test_prod_float() {
        let tensor_1 = TestTensor::<2>::from([[-5.0, 1.0, 2.0], [3.0, 4.0, 5.0]]);

        let output = tensor_1.prod();

        output
            .into_data()
            .assert_eq(&TensorData::from([-600.0]), false);
    }
}
