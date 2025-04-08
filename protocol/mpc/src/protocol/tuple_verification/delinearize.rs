use protocol::LargeField;

use crate::Context;

impl Context{
    // This function will be used to compress the multiplication tuples
    // It will take the shares of a, b, and the output and compress them into a single representation
    pub async fn delinearize_mult_tuples(&mut self){
        // Here we will implement the logic for compressing the multiplication tuples
        // This might involve some form of serialization or aggregation of the shares
        // Initiate the random mask generation for the last level
        log::info!("Initiating verification process: Preparing a random mask and tossing a common coin");
        let random_a_share = self.rand_sharings_state.rand_sharings_mult.pop_front().unwrap();
        let random_b_share = self.rand_sharings_state.rand_sharings_mult.pop_front().unwrap();

        //let vec_a_share = vec![vec![random_a_share]];
        //let vec_b_share = vec![vec![random_b_share]];

        self.verf_state.random_mask.0 = Some(random_a_share);
        self.verf_state.random_mask.1 = Some(random_b_share);

        //self.choose_multiplication_protocol(vec_a_share, vec_b_share, self.delinearization_depth).await;
        self.toss_common_coin(self.delinearization_depth).await;
    }

    pub async fn verify_coin_toss_deserialization(&mut self){
        if !self.verf_state.ex_compr_state.contains_key(&self.delinearization_depth){
            return;
        }
        let ex_compr_state = self.verf_state.ex_compr_state.get_mut(&self.delinearization_depth).unwrap();
        if ex_compr_state.coin_output.is_none(){
            return;
        }
        let coin_value = ex_compr_state.coin_output.clone().unwrap();
        let _depth_factor = self.compression_factor;
        // Reduce multiplicative depth by a factor of k in each iteration
        // Collect all multiplication tuples so far
        let mut x_values = Vec::new();
        let mut y_values = Vec::new();
        let mut mult_values = Vec::new();

        for i in 0..self.max_depth+1{
            if !self.verf_state.mult_tuples.contains_key(&i){
                continue;
            }
            let verf_state = self.verf_state.mult_tuples.get(&i).unwrap();
            x_values.extend(verf_state.0.clone());
            y_values.extend(verf_state.1.clone());
            mult_values.extend(verf_state.2.clone());
        }
        log::info!("Initiating verification process for {} multiplication tuples: x: {}, y: {}, mult: {}",x_values.len(), x_values.len(), y_values.len(), mult_values.len());
        if x_values.len() != y_values.len() || x_values.len() != mult_values.len() || x_values.len() == 0{
            log::error!("Invalid number of shares for delinearization {} {} {}, abandoning process", x_values.len(), y_values.len(), mult_values.len());
            return;
        }
        let mut r_iter = LargeField::one();
        for (x,mult) in x_values.iter_mut().zip(mult_values.iter_mut()){
            *x *= r_iter;
            *mult *= r_iter;
            r_iter *= coin_value;
        }
        log::info!("Multiplication tuples after coin toss: x: {}, y: {}, mult: {}",x_values.len(), y_values.len(), mult_values.len());
        // Compress shares with dimension reduction factor k
        let summed_mult_value: LargeField = mult_values.into_iter().sum();
        self.init_compression_level(x_values, y_values, summed_mult_value, self.delinearization_depth +2).await;
    }
}