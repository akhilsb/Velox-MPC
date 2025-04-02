use crate::Context;

impl Context{
    // This function will be used to compress the multiplication tuples
    // It will take the shares of a, b, and the output and compress them into a single representation
    pub async fn delinearize_mult_tuples(&mut self){
        // Here we will implement the logic for compressing the multiplication tuples
        // This might involve some form of serialization or aggregation of the shares
        
        let _depth_factor = self.compression_factor;
        // Reduce multiplicative depth by a factor of k in each iteration
        // Collect all multiplication tuples so far
        let mut x_values = Vec::new();
        let mut y_values = Vec::new();
        let mut mult_values = Vec::new();

        for i in 0..self.max_depth{
            if !self.verf_state.mult_tuples.contains_key(&i){
                continue;
            }
            let verf_state = self.verf_state.mult_tuples.get(&i).unwrap();
            x_values.extend(verf_state.0.clone());
            y_values.extend(verf_state.1.clone());
            mult_values.extend(verf_state.2.clone());
        }

        // Sample a coin first here
        self.toss_common_coin(self.delinearization_depth).await;
    }

    pub async fn handle_coin_toss_deserialization(&mut self){
        
    }
}