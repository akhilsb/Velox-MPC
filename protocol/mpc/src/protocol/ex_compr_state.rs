use lambdaworks_math::polynomial::Polynomial;
use protocol::LargeField;

pub struct ExComprState{
    pub depth: usize,
    pub x_sharings: Vec<Vec<LargeField>>,
    pub y_sharings: Vec<Vec<LargeField>>,
    pub mult_sharings: Vec<LargeField>,

    pub x_polys: Vec<Polynomial<LargeField>>,
    pub y_polys: Vec<Polynomial<LargeField>>,
    pub h_poly: Polynomial<LargeField>,

    pub extended_x_sharings: Vec<Vec<LargeField>>,
    pub extended_y_sharings: Vec<Vec<LargeField>>,
    pub extended_mult_sharings: Vec<LargeField>,

    // Tuple represents ordered evaluation indices as well as the shares
    pub coin_toss_shares: (Vec<LargeField>, Vec<LargeField>),
    pub coin_output: LargeField
}

impl ExComprState{
    pub fn new(depth: usize) -> Self {
        ExComprState{
            depth,
            x_sharings: Vec::new(),
            y_sharings: Vec::new(),
            mult_sharings: Vec::new(),

            x_polys: Vec::new(),
            y_polys: Vec::new(),
            h_poly: Polynomial{coefficients: vec![LargeField::zero();1]}, // Initialize with a zero polynomial, will be set later

            extended_x_sharings: Vec::new(),
            extended_y_sharings: Vec::new(),
            extended_mult_sharings: Vec::new(),

            coin_toss_shares: (Vec::new(), Vec::new()), // Initialize with empty vectors for coin toss shares
            coin_output: LargeField::zero() // Initialize with a zero value, will be set later
        }
    }    
}