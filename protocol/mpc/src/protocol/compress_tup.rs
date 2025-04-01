use lambdaworks_math::polynomial::Polynomial;
use protocol::LargeField;
use rayon::prelude::{IntoParallelIterator, ParallelIterator, IntoParallelRefIterator};

use crate::Context;

use super::ex_compr_state::ExComprState;

impl Context{
    // This function will be used to compress the multiplication tuples
    // It will take the shares of a, b, and the output and compress them into a single representation
    pub fn compress_multiplication_tuples(&mut self) -> Result<(), String> {
        // Here we will implement the logic for compressing the multiplication tuples
        // This might involve some form of serialization or aggregation of the shares
        
        let depth_factor = self.compression_factor;
        // Reduce multiplicative depth by a factor of k in each iteration

        Ok(())
    }

    // This function takes a two-layered vector: 
    // First layer is a vector of tuples
    // Second layer is encompasses a set of k vectors.  
    pub async fn ex_compression_tuples(&mut self, depth: usize) {
        // create polynomials on x and y
        if !self.verf_state.ex_compr_state.contains_key(&depth){
            return;
            //self.verf_state.add_compression_level_state(depth, x_vectors.clone(), y_vectors.clone(), mult_vec.clone());
        }
        let ex_compr_state = self.verf_state.ex_compr_state.get_mut(&depth).expect("ExComprState should exist for the given depth");
        
        let x_vectors = ex_compr_state.x_sharings.clone(); // This should be a vector of vectors of shares for x
        let y_vectors = ex_compr_state.y_sharings.clone(); // This should be a vector of vectors of shares for y
        let mult_vec = ex_compr_state.mult_sharings.clone(); // This should be a vector of shares for the multiplication results

        if x_vectors.len() != y_vectors.len() || x_vectors.len() != mult_vec.len() {
            log::error!("Ex_compr: X, Y, and Z vectors must be of the same length, returning multiplication");
            return; // Handle error: x and y vectors must be of the same length
        }

        if !ex_compr_state.extended_mult_sharings.is_empty() && !ex_compr_state.extended_x_sharings.is_empty(){
            // Directly go to the extended protocol now. 
            // TODO: something here
        }

        let (first_set_eval_points, second_set_eval_points) = 
            Self::gen_evaluation_points_ex_compr(x_vectors.len());
        
        let mut x_polynomial_evaluations_vector = vec![vec![LargeField::zero();x_vectors.len()];x_vectors[0].len()]; // This will hold the polynomial evaluations for each x vector
        let mut y_polynomial_evaluations_vector = vec![vec![LargeField::zero();x_vectors.len()];x_vectors[0].len()]; // This will hold the polynomial evaluations for each x vector
        for (x_vec, y_vec) in x_vectors.iter().zip(y_vectors.iter()){
            for ((outer_index,x_point),y_point) in x_vec.iter().enumerate().zip(y_vec.iter()){
                x_polynomial_evaluations_vector[outer_index].push(x_point.clone());
                y_polynomial_evaluations_vector[outer_index].push(y_point.clone());
            } 
        }

        let x_polynomials: Vec<Polynomial<LargeField>> = x_polynomial_evaluations_vector.into_par_iter().map(|evaluations| {
            return Polynomial::interpolate(&first_set_eval_points, &evaluations).unwrap();
        }).collect();
        let y_polynomials: Vec<Polynomial<LargeField>> = y_polynomial_evaluations_vector.into_par_iter().map(|evaluations| {
            return Polynomial::interpolate(&second_set_eval_points, &evaluations).unwrap();
        }).collect();

        // Evaluate polynomials on second set of points and collect them.

        let mut x_poly_evals_ss = vec![vec![LargeField::zero(); x_vectors[0].len()];x_vectors.len()];
        let mut y_poly_evals_ss = vec![vec![LargeField::zero(); y_vectors[0].len()];y_vectors.len()];

        for (x_poly, y_poly) in x_polynomials.iter().zip(y_polynomials.iter()) {
            // Evaluate on the second set of points
            let x_eval = second_set_eval_points.par_iter().map(|point| x_poly.evaluate(point)).collect::<Vec<LargeField>>();
            let y_eval = second_set_eval_points.par_iter().map(|point| y_poly.evaluate(point)).collect::<Vec<LargeField>>();

            // Store evaluations in respective vectors
            for (outer_index, (x_val, y_val)) in x_eval.into_iter().zip(y_eval.into_iter()).enumerate() {
                x_poly_evals_ss[outer_index].push(x_val);
                y_poly_evals_ss[outer_index].push(y_val);
            }
        }

        ex_compr_state.x_polys.extend(x_polynomials);
        ex_compr_state.y_polys.extend(y_polynomials);

        ex_compr_state.extended_x_sharings.extend(x_poly_evals_ss.clone());
        ex_compr_state.extended_y_sharings.extend(y_poly_evals_ss.clone()); // Store the evaluations in the state for future reference
        // Send these tuples to multiplication
        // Remember, asynchrony can cause extended_mult_sharings to be filled first as well. 
        let mult_sharings_filled = ex_compr_state.mult_sharings.len() > 0;
        if mult_sharings_filled{
            //self.handle_ex_mult_termination(depth+1, ).await;
        }
        else{
            self.choose_multiplication_protocol(x_poly_evals_ss, y_poly_evals_ss, depth+1).await;
        }
    }

    pub async fn handle_ex_mult_termination(&mut self, depth: usize, mult_result: Vec<LargeField>){
        if depth % 2 == 0{
            // This is the first level of ex_mult termination, initiate second level of ex_mult at this depth here
            let ex_compr_state = self.verf_state.ex_compr_state.entry(depth).or_insert_with(|| ExComprState::new(depth));
            ex_compr_state.mult_sharings.extend(mult_result.clone());
            self.ex_compression_tuples(depth).await;
        }
        else{
            // This is the second level of ex_mult termination, initiate further compression here
            let depth_state_ex_compr = depth - 1;
            let ex_compr_state = self.verf_state.ex_compr_state.entry(depth_state_ex_compr).or_insert_with(|| ExComprState::new(depth));
            ex_compr_state.extended_mult_sharings.extend(mult_result.clone()); // Store the multiplication results for the next round of compression
        }
    }

    pub async fn handle_level_termination(&mut self, depth: usize){
        if !self.verf_state.ex_compr_state.contains_key(&depth) {
            // This means we haven't even started the ex_compression at this depth, return early
            return;
        }
        let ex_compr_state = self.verf_state.ex_compr_state.get_mut(&depth).unwrap();
        if ex_compr_state.x_polys.is_empty() ||
            ex_compr_state.y_polys.is_empty() ||
            ex_compr_state.extended_x_sharings.is_empty() || 
            ex_compr_state.extended_y_sharings.is_empty() || 
            ex_compr_state.extended_mult_sharings.is_empty() {
            // We haven't filled the extended sharings yet, return early
            log::error!("handle_level_termination: Not enough data to proceed with level termination at depth {}. x_polys: {}, y_polys: {}, extended_x_sharings: {}, extended_y_sharings: {}, extended_mult_sharings: {}",
                depth,
                ex_compr_state.x_polys.len(),
                ex_compr_state.y_polys.len(),
                ex_compr_state.extended_x_sharings.len(),
                ex_compr_state.extended_y_sharings.len(),
                ex_compr_state.extended_mult_sharings.len());
            return;
        }

        // Interpolate h polynomial
        let mut h_shares = ex_compr_state.mult_sharings.clone();
        h_shares.extend(ex_compr_state.extended_mult_sharings.clone()); // Include the multiplication results for interpolation

        let (mut evaluation_points,evaluation_points_2) = Self::gen_evaluation_points_ex_compr(ex_compr_state.mult_sharings.len());
        evaluation_points.extend(evaluation_points_2);

        let h_polynomial = Polynomial::interpolate(&evaluation_points, &h_shares).unwrap();
        log::info!("Interpolated H polynomial with degree {} at ExCompr at depth {}", h_polynomial.degree(), depth);

        // Evaluate x,y,h polynomials at a random point to get final value at this level
        ex_compr_state.h_poly = h_polynomial.clone();

        // Toss coin here
        
    }

    // pub async fn on_ex_mult_terminating(self: &mut Context, z_i_shares: Vec<Vec<LargeField>>) {
    //     assert_eq!(z_i_shares.len(), self.N); // self.N / (self.num_faults + 1)
    //     assert_eq!(z_i_shares[0].len(), 2*self.num_faults + 1);
    //     assert!(z_i_shares.windows(2).all(|w| w[0].len() == w[1].len()));

    //     // flatten z_i_shares
    //     let z_i_shares_flat: Vec<LargeField> = z_i_shares.into_iter().flatten().collect_vec();
    //     assert_eq!(z_i_shares_flat.len(), self.N);

    //     // compute coefficients of h(.) from points [(alpha_1, z_1), (alpha_1, z_1), ..., (alpha_{2N-1}, z_{2N-1})]
    //     assert_eq!(self.alpha_i.len(), z_i_shares_flat.len());
    //     assert_eq!(self.alpha_i.len(), 2*self.N - 1);
    //     let shares: Vec<(LargeField, LargeField)> = zip(
    //         self.alpha_i.clone(), z_i_shares_flat.clone()).collect_vec();
    //     let h_coefficients = interpolate_polynomial(shares);

    //     let r: LargeField = self.PiCoin();

    //     if any(&self.alpha_i, |alpha| *alpha == r) {
    //         // self.terminate() // FAIL // TODO
    //     } else {
    //         let mut f: Vec<LargeField> = Vec::new();
    //         let mut g: Vec<LargeField> = Vec::new();
    //         let h:  LargeField = evaluate_polynomial_from_coefficients_at_position(h_coefficients.clone(), r);
    //         assert_eq!(self.f_vec_coefficient_shares.len(), self.g_vec_coefficient_shares.len());
    //         for i in 0..self.f_vec_coefficient_shares.len() {
    //             let f_val = evaluate_polynomial_from_coefficients_at_position(self.f_vec_coefficient_shares[i].clone(), r);
    //             let g_val = evaluate_polynomial_from_coefficients_at_position(self.g_vec_coefficient_shares[i].clone(), r);
    //             f.push(f_val);
    //             g.push(g_val);
    //         }

    //         // self.terminate() // f, g, h // TODO
    //     }

    // }
    
    pub fn PiCoin(self: &mut Context) -> LargeField {
        return LargeField::zero(); // TODO: call actual implementation!
    }

    pub fn gen_evaluation_points_ex_compr(poly_def_points_count: usize)-> (Vec<LargeField>, Vec<LargeField>) {
        let mut first_set = Vec::with_capacity(poly_def_points_count);
        let mut second_set = Vec::with_capacity(poly_def_points_count);

        for i in 1..poly_def_points_count+1{
            first_set.push(LargeField::from(i as u64)); // Generate first set of evaluation points
            second_set.push(LargeField::from((i+poly_def_points_count) as u64));    
        }
        (first_set, second_set)
    }
}