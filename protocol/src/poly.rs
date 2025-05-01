use std::{collections::HashMap, ops::{Mul, Sub, Add}};

use crypto::hash::do_hash;
use lambdaworks_math::{unsigned_integer::element::UnsignedInteger, polynomial::Polynomial, field::fields::{montgomery_backed_prime_fields::MontgomeryBackendPrimeField, fft_friendly::stark_252_prime_field::MontgomeryConfigStark252PrimeField}};
use rand::random;
use rand_chacha::ChaCha20Rng;
use rand_core::{SeedableRng, RngCore};
use rayon::prelude::{IntoParallelIterator, ParallelIterator, IntoParallelRefIterator};
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
    degree: usize,
    shares_total: usize,
) -> (Vec<Vec<LargeField>>, 
    Vec<Polynomial<LargeField>>
){

    // The first evaluation is always at 0
    let mut evaluation_points = Vec::new();
    evaluation_points.push(LargeField::new(UnsignedInteger::from(0u64)));
    for i in 0..degree{
        evaluation_points.push(LargeField::new(UnsignedInteger::from((i+1) as u64)));
    }
    
    // Generate coefficients of polynomial and then evaluate the polynomial at n points
    let coefficients: Vec<Polynomial<LargeField>> = evaluations_prf.into_par_iter().map(|evals| {
        return Polynomial::interpolate(evaluation_points.as_slice(), evals.as_slice()).unwrap()
    }).collect();

    // Evaluate the polynomial at n points
    let evaluations_full = coefficients.par_iter().map(|polynomial|{
        let mut eval_vec_ind = Vec::new();
        for index in 0..shares_total{
            eval_vec_ind.push(polynomial.evaluate(&LargeField::new(UnsignedInteger::from((index+1) as u64))));
        }
        return eval_vec_ind;
    }).collect();
    (evaluations_full,coefficients)
}

pub async fn generate_evaluation_points_opt(
    evaluations_prf: Vec<Vec<LargeField>>, 
    degree: usize,
    shares_total: usize,
) -> (Vec<Vec<LargeField>>, 
    Vec<Polynomial<LargeField>>
){

    // The first evaluation is always at 0
    let mut evaluation_points = Vec::new();
    evaluation_points.push(LargeField::new(UnsignedInteger::from(0u64)));
    for i in 0..degree{
        evaluation_points.push(LargeField::new(UnsignedInteger::from((i+1) as u64)));
    }
    
    // Generate vandermonde matrix
    let vandermonde = vandermonde_matrix(evaluation_points.clone());
    let inverse_vandermonde = inverse_vandermonde(vandermonde);

    let coefficients : Vec<Polynomial<LargeField>> = evaluations_prf.into_par_iter().map(|evals|{
        let coefficients = matrix_vector_multiply(&inverse_vandermonde, &evals);
        return Polynomial::new(&coefficients);
    }).collect();
    // Generate coefficients of polynomial and then evaluate the polynomial at n points
    // let coefficients: Vec<Polynomial<LargeField>> = evaluations_prf.into_par_iter().map(|evals| {
    //     return Polynomial::interpolate(evaluation_points.as_slice(), evals.as_slice()).unwrap()
    // }).collect();

    // Evaluate the polynomial at n points
    let evaluations_full = coefficients.par_iter().map(|polynomial|{
        let mut eval_vec_ind = Vec::new();
        for index in 0..shares_total{
            eval_vec_ind.push(polynomial.evaluate(&LargeField::new(UnsignedInteger::from((index+1) as u64))));
        }
        return eval_vec_ind;
    }).collect();
    (evaluations_full,coefficients)
}

pub async fn generate_evaluation_points_fft(
    secrets: Vec<LargeField>,
    degree_poly: usize,
    shares_total: usize,
)-> (Vec<Vec<LargeField>>, 
    Vec<Polynomial<LargeField>>
){
    // For FFT evaluations, first sample coefficients of polynomial and then interpolate all n points
    let coefficients: Vec<Polynomial<LargeField>> = secrets.into_par_iter().map(|secret| {
        let mut coeffs_single_poly = Vec::new();
        coeffs_single_poly.push(secret);
        for _ in 0..degree_poly{
            coeffs_single_poly.push(rand_field_element());
        }
        return Polynomial::new(&coeffs_single_poly);
    }).collect();

    let evaluations = coefficients.par_iter().map(|poly_coeffs|{
        let poly_evaluations_fft = Polynomial::evaluate_fft::<MontgomeryBackendPrimeField<MontgomeryConfigStark252PrimeField, 4>>(poly_coeffs, 1, Some(shares_total)).unwrap();
        poly_evaluations_fft
    }).collect();
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

pub fn check_if_all_points_lie_on_degree_x_polynomial(eval_points: Vec<LargeField>, polys_vector: Vec<Vec<LargeField>>, degree: usize) -> (bool,Option<Vec<Polynomial<LargeField>>>){
    //log::info!("Checking evaluations on points :{:?}, eval_points: {:?}", eval_points, polys_vector);
    let inverse_vandermonde = inverse_vandermonde(vandermonde_matrix(eval_points[0..degree].to_vec()));
    let polys = polys_vector.into_par_iter().map(|points| {
        let coeffs = matrix_vector_multiply(&inverse_vandermonde, &points[0..degree].to_vec());
        let polynomial = Polynomial::new(&coeffs);
        let all_points_match =  eval_points[degree..].iter().zip(points[degree..].iter()).map(|(eval_point, share)|{
            return polynomial.evaluate(eval_point) == *share;
        }).fold(true, |acc,x| acc && x);

        if all_points_match{
            Some(polynomial)
        }
        else{
            None
        }
    }).fold(|| Vec::new(), |mut acc_vec, vec: Option<Polynomial<LargeField>>|{
        acc_vec.push(vec);
        acc_vec
    }).reduce(|| Vec::new(), |mut acc_vec, vec: Vec<Option<Polynomial<LargeField>>>|{
        acc_vec.extend(vec);
        acc_vec
    });
    let all_polys_positive = polys.par_iter().all(|poly| poly.is_some());
    if all_polys_positive{
        let polys_vec = polys.into_iter().map(|x| x.unwrap()).collect();
        (true, Some(polys_vec))
    }
    else{
        (false, None)
    }
}


/// Constructs the Vandermonde matrix for a given set of x-values.
pub fn vandermonde_matrix(x_values: Vec<LargeField>) -> Vec<Vec<LargeField>> {
    let n = x_values.len();
    let mut matrix = vec![vec![LargeField::zero(); n]; n];

    for (row, x) in x_values.iter().enumerate() {
        let mut value = LargeField::one();
        for col in 0..n {
            matrix[row][col] = value.clone();
            value = value.mul(x);
        }
    }

    matrix
}

/// Computes the inverse of a Vandermonde matrix modulo prime using Gaussian elimination.
pub fn inverse_vandermonde(matrix: Vec<Vec<LargeField>>) -> Vec<Vec<LargeField>> {
    let n = matrix.len();
    let mut augmented = matrix.clone();

    // Extend the matrix with an identity matrix on the right
    for i in 0..n {
        augmented[i].extend((0..n).map(|j| if i == j { LargeField::one() } else { LargeField::zero() }));
    }

    // Perform Gaussian elimination
    for col in 0..n {
        // Normalize pivot row
        let inv = &augmented[col][col].inv().unwrap();
        for k in col..2 * n {
            augmented[col][k] = augmented[col][k].mul(inv);
        }

        // Eliminate other rows
        for row in 0..n {
            if row != col {
                let factor = augmented[row][col].clone();
                for k in col..2 * n {
                    augmented[row][k] = augmented[row][k].sub(factor.mul(augmented[col][k]));
                }
            }
        }
    }

    // Extract the right half as the inverse
    augmented
        .into_iter()
        .map(|row| row[n..2 * n].to_vec())
        .collect()
}

pub fn matrix_vector_multiply(
    matrix: &Vec<Vec<LargeField>>,
    vector: &Vec<LargeField>,
) -> Vec<LargeField> {
    matrix
        .par_iter()
        .map(|row| {
            row.iter()
                .zip(vector)
                .fold(LargeField::zero(), |sum, (a, b)| sum.add(a.mul(b)))
        })
        .collect()
}