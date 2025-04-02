use lambdaworks_math::{polynomial::Polynomial};
use protocol::{LargeField};
use rayon::prelude::{IntoParallelIterator, ParallelIterator, IntoParallelRefIterator};

use crate::{Context};

use super::ex_compr_state::ExComprState;

impl Context{
    // This method starts compression from the second level onwards
    pub async fn start_compression_level(&mut self, x_vector: Vec<LargeField>, y_vector: Vec<LargeField>, agg_val: LargeField, depth: usize){
        // Split into chunks for compression
        let num_chunks = self.compression_factor;
        let mut x_vec_chunks: Vec<Vec<LargeField>> = x_vector.chunks(num_chunks).into_iter().map(|chunk| chunk.to_vec()).collect();
        let mut y_vec_chunks: Vec<Vec<LargeField>> = y_vector.chunks(num_chunks).into_iter().map(|chunk| chunk.to_vec()).collect();
        let mult_value = agg_val;

        if !self.verf_state.ex_compr_state.contains_key(&depth){
            let ex_compr_state = ExComprState::new(depth);
            self.verf_state.ex_compr_state.insert(depth, ex_compr_state);
        }

        let ex_compr_state = self.verf_state.ex_compr_state.get_mut(&depth).unwrap();
        // Save the first tuple in the Structure
        let first_x_chunk = x_vec_chunks.pop().unwrap();
        let first_y_chunk = y_vec_chunks.pop().unwrap();
        ex_compr_state.rem_mult_tup = Some((first_x_chunk, first_y_chunk, mult_value));

        // Save the multiplication results in the structure
        ex_compr_state.x_sharings.extend(x_vec_chunks.clone());
        ex_compr_state.y_sharings.extend(y_vec_chunks.clone());

        // Multiply these tuples using ex_mult
        self.choose_multiplication_protocol(x_vec_chunks, y_vec_chunks, depth).await;
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
            self.handle_level_termination(depth).await;
            return;
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

        ex_compr_state.x_polys = Some(x_polynomials);
        ex_compr_state.y_polys = Some(y_polynomials);

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
            self.handle_level_termination(depth_state_ex_compr).await;
        }
    }

    pub async fn handle_level_termination(&mut self, depth: usize){
        if !self.verf_state.ex_compr_state.contains_key(&depth) {
            // This means we haven't even started the ex_compression at this depth, return early
            return;
        }
        let ex_compr_state = self.verf_state.ex_compr_state.get_mut(&depth).unwrap();
        if ex_compr_state.x_polys.is_none() ||
            ex_compr_state.y_polys.is_none() ||
            ex_compr_state.extended_x_sharings.is_empty() || 
            ex_compr_state.extended_y_sharings.is_empty() || 
            ex_compr_state.extended_mult_sharings.is_empty() {
            // We haven't filled the extended sharings yet, return early
            log::error!("handle_level_termination: Not enough data to proceed with level termination at depth {}. x_polys: {:?}, y_polys: {:?}, extended_x_sharings: {}, extended_y_sharings: {}, extended_mult_sharings: {}",
                depth,
                0,
                0,
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
        ex_compr_state.h_poly = Some(h_polynomial.clone());

        // Toss coin here
        self.toss_common_coin(depth).await;
    }

    pub async fn handle_coin_termination(&mut self, depth: usize) {
        let ex_compr_state = self.verf_state.ex_compr_state.get_mut(&depth).unwrap();
        if ex_compr_state.h_poly.is_none() || ex_compr_state.x_polys.is_none() || ex_compr_state.y_polys.is_none() || ex_compr_state.coin_output.is_none() {
            log::warn!("handle_coin_termination: h_poly is None at depth {}. Cannot proceed with coin termination.", depth);
            return;
        }
        let h_polynomial = ex_compr_state.h_poly.as_ref().unwrap();
        let x_poly_vec = ex_compr_state.x_polys.as_ref().unwrap();
        let y_poly_vec = ex_compr_state.y_polys.as_ref().unwrap();

        let coin_eval_point = ex_compr_state.coin_output.clone().unwrap();
        let h_point = h_polynomial.evaluate(&coin_eval_point);
        let x_points: Vec<LargeField> = x_poly_vec.par_iter().map(|poly| poly.evaluate(&coin_eval_point)).collect();
        let y_points: Vec<LargeField> = y_poly_vec.par_iter().map(|poly| poly.evaluate(&coin_eval_point)).collect();

        self.start_compression_level(x_points, y_points, h_point, depth+2).await;
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