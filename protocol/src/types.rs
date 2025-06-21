use lambdaworks_math::{field::{fields::{montgomery_backed_prime_fields::MontgomeryBackendPrimeField}, element::FieldElement}};
use lambdaworks_math::field::fields::fft_friendly::stark_252_prime_field::{MontgomeryConfigStark252PrimeField};

// pub type LargeField = FieldElement<MontgomeryBackendPrimeField<MontgomeryConfigStark252PrimeField, 4>>;
// pub type FieldType = MontgomeryBackendPrimeField<MontgomeryConfigStark252PrimeField, 4>;
// pub type LargeFieldSer = [u8;32];

// pub const FIELD_DIV_2: &str = "400000000000008800000000000000000000000000000000000000000000000";
// use lambdaworks_math::{fft::cpu::roots_of_unity::get_powers_of_primitive_root, field::traits::RootsConfig};
// pub fn gen_roots_of_unity(n: usize) -> Vec<LargeField> {
//     let len = n.next_power_of_two();
//     let order = len.trailing_zeros();
//     get_powers_of_primitive_root(order.into(), len, RootsConfig::Natural).unwrap()
// }

use lambdaworks_math::elliptic_curve::short_weierstrass::curves::bn_254::field_extension::BN254PrimeField;
pub type LargeField = FieldElement<BN254PrimeField>;
pub type FieldType = MontgomeryBackendPrimeField<BN254PrimeField, 4>;
pub type LargeFieldSer = Vec<u8>;

pub const FIELD_DIV_2: &str = "183227397098D014DC2822DB40C0AC2ECBC0B548B438E5469E10460B6C3E7EA3";
// temporary fix

pub fn gen_roots_of_unity(n: usize) -> Vec<LargeField> {
    (1..n+1).into_iter().map(|x| LargeField::from(x as u64)).collect()
}

// use lambdaworks_math::elliptic_curve::short_weierstrass::curves::bn_254::field_extension::BN254PrimeField;
// pub type LargeFieldBN = FieldElement<BN254PrimeField>;
// pub type FieldType = MontgomeryBackendPrimeField<BN254PrimeField, 4>;
// pub type LargeFieldSer = Vec<u8>;
// temporary fix

// pub fn gen_roots_of_unity(n: usize) -> Vec<LargeField> {
//     (1..n+1).into_iter().map(|x| LargeField::from(x as u64)).collect()
// }

pub type FieldTypeFFT = MontgomeryBackendPrimeField<MontgomeryConfigStark252PrimeField, 4>;


// Shares, nonce polynomial, blinding_nonce polynomial
pub type AvssShare =  (Vec<LargeFieldSer>, LargeFieldSer, LargeFieldSer);