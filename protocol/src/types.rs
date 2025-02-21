use lambdaworks_math::field::{fields::fft_friendly::stark_252_prime_field::Stark252PrimeField, element::FieldElement};

pub type LargeField = FieldElement<Stark252PrimeField>;
pub type LargeFieldSer = [u8;32];