#[burn_tensor_testgen::testgen(recip)]
mod tests {
    use super::*;
    use burn_tensor::{Tensor, TensorData};
    use burn_tensor::{Tolerance, ops::FloatElem};
    type FT = FloatElem<TestBackend>;

    #[test]
    fn should_support_recip_ops() {
        let data = TensorData::from([[0.5, 1.0, 2.0], [3.0, -4.0, -5.0]]);
        let tensor = TestTensor::<2>::from_data(data, &Default::default());

        let output = tensor.recip();
        let expected = TensorData::from([[2.0, 1.0, 0.5], [0.33333, -0.25, -0.2]]);

        output
            .into_data()
            .assert_approx_eq::<FT>(&expected, Tolerance::default());
    }
}
