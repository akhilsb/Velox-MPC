use lambdaworks_math::field::{fields::{fft_friendly::stark_252_prime_field::{MontgomeryConfigStark252PrimeField}, montgomery_backed_prime_fields::MontgomeryBackendPrimeField}, element::FieldElement};

pub type LargeField = FieldElement<MontgomeryBackendPrimeField<MontgomeryConfigStark252PrimeField, 4>>;
pub type FieldType = MontgomeryBackendPrimeField<MontgomeryConfigStark252PrimeField, 4>;

//pub type LargeField = Secp256k1PrimeField;
//pub type FieldType = Secp256k1PrimeField;

pub type LargeFieldSer = [u8;32];

// Shares, nonce polynomial, blinding_nonce polynomial
pub type AvssShare =  (Vec<LargeFieldSer>, LargeFieldSer, LargeFieldSer);