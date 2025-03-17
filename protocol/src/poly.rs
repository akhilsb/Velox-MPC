use std::collections::HashMap;

use crypto::hash::do_hash;
use lambdaworks_math::{unsigned_integer::element::UnsignedInteger, polynomial::Polynomial, field::fields::{montgomery_backed_prime_fields::MontgomeryBackendPrimeField, fft_friendly::stark_252_prime_field::MontgomeryConfigStark252PrimeField}};
use rand::random;
use rand_chacha::ChaCha20Rng;
use rand_core::{SeedableRng, RngCore};
use types::Replica;

use crate::LargeField;

pub fn sample_polynomials_from_prf(
    secrets: Vec<LargeField>, 
    sec_key_map: HashMap<Replica, Vec<u8>>, 
    degree: usize,
    is_nonce: bool,
    nonce: u8
)-> Vec<Vec<LargeField>>{
    let tot_evaluations = secrets.len();
    let mut evaluations = Vec::new();
    for secret in secrets{
        evaluations.push(vec![secret]);
    }
    for i in 0..degree{
        let mut sec_key = sec_key_map.get(&(i as Replica)).unwrap().clone();
        if is_nonce{
            sec_key.push(nonce);
        }
        let samples = pseudorandom_lf(&sec_key, tot_evaluations);
        for (i,sample) in samples.into_iter().enumerate() {
            evaluations[i].push(sample);
        }
    }
    evaluations
}

pub async fn generate_evaluation_points(
    evaluations_prf: Vec<Vec<LargeField>>, 
    num_faults: usize,
    num_nodes: usize,
) -> (Vec<Vec<LargeField>>, 
    Vec<Polynomial<LargeField>>
){

    // The first evaluation is always at 0
    let mut evaluation_points = Vec::new();
    evaluation_points.push(LargeField::new(UnsignedInteger::from(0u64)));
    for i in 0..num_faults{
        evaluation_points.push(LargeField::new(UnsignedInteger::from((i+1) as u64)));
    }
    
    // Generate coefficients of polynomial and then evaluate the polynomial at n points
    let coefficients: Vec<Polynomial<LargeField>> = evaluations_prf.into_iter().map(|evals| {
        return Polynomial::interpolate(evaluation_points.as_slice(), evals.as_slice()).unwrap()
    }).collect();

    // Evaluate the polynomial at n points
    let mut evaluations_full = Vec::new();
    for polynomial in coefficients.iter(){
        let mut eval_vec_ind = Vec::new();
        for index in 0..num_nodes{
            eval_vec_ind.push(polynomial.evaluate(&LargeField::new(UnsignedInteger::from((index+1) as u64))));
        }
        evaluations_full.push(eval_vec_ind);
    }
    (evaluations_full,coefficients)
}

pub async fn generate_evaluation_points_fft(
    secrets: Vec<LargeField>,
    num_faults: usize,
    num_nodes: usize,
)-> (Vec<Vec<LargeField>>, 
    Vec<Polynomial<LargeField>>
){
    // For FFT evaluations, first sample coefficients of polynomial and then interpolate all n points
    let mut coefficients = Vec::new();
    for secret in secrets.clone().into_iter(){
        let mut coeffs_single_poly = Vec::new();
        coeffs_single_poly.push(secret);
        for _ in 0..num_faults{
            coeffs_single_poly.push(rand_field_element());
        }
        coefficients.push(Polynomial::new(&coeffs_single_poly));
    }

    let mut evaluations = Vec::new();
    for poly_coeffs in coefficients.iter(){
        let poly_evaluations_fft = Polynomial::evaluate_fft::<MontgomeryBackendPrimeField<MontgomeryConfigStark252PrimeField, 4>>(poly_coeffs, 1, Some(num_nodes)).unwrap();
        // This vector has 3t+3 elements. Trim the last 2 elements
        evaluations.push(poly_evaluations_fft);
    }
    (evaluations, coefficients)
}

pub fn pseudorandom_lf(rng_seed: &[u8], num: usize)->Vec<LargeField>{
    let mut rng = ChaCha20Rng::from_seed(do_hash(rng_seed));
    let mut random_numbers: Vec<LargeField> = Vec::new();
    for _i in 0..num{
        let mut limbs = [0u64;4];
        for j in 0..4{
            limbs[j] = rng.next_u64();
        }
        let bigint_rand = UnsignedInteger{ 
            limbs: limbs
        };
        random_numbers.push(LargeField::new( bigint_rand));
    }
    random_numbers
}

pub fn rand_field_element() -> LargeField {
    let rand_big = UnsignedInteger { limbs: random() };
    LargeField::new(rand_big)
}


pub fn interpolate_shares( mut secret_key: Vec<u8>, num_shares: usize, is_nonce: bool, padding: u8) -> Vec<LargeField>{
    if is_nonce{
        secret_key.push(padding);
    }
    let prf_values = pseudorandom_lf(&secret_key, num_shares);
    prf_values
}