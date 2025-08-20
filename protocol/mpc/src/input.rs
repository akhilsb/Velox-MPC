use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::anyhow;
use protocol::LargeField;

/// Reads first k lines from the primary file, or fallback file if primary doesn't exist
/// Validates that each line can be represented as a 250-bit binary string
pub fn read_input_from_files(
    primary_file_path: &str,
    fallback_file_path: &str,
    k: usize,
) -> Result<Vec<LargeField>,anyhow::Error> {
    // Check which file to use
    let file_path = if Path::new(primary_file_path).exists() {
        log::info!("Primary file exists, reading from: {}", primary_file_path);
        primary_file_path
    } else {
        log::info!("Primary file doesn't exist, reading from fallback: {}", fallback_file_path);
        fallback_file_path
    };

    // Open and read the file
    let file = File::open(file_path)
        .map_err(|e| anyhow!("Failed to open file {}: {}", file_path, e))?;
    
    let reader = BufReader::new(file);
    let mut converted_fes = Vec::new();
    let mut line_count = 0;

    for line_result in reader.lines() {
        if line_count >= k {
            break;
        }

        let line = line_result
            .map_err(|e| anyhow!("Failed to read line {}: {}", line_count + 1, e))?;
        
        // Validate that the line can be represented as a 250-bit binary string
        let conversion_output = convert_string_to_large_field(&line);
        if conversion_output.is_none() {
            return Err(anyhow!(
                "Line {} cannot be represented as a 250-bit binary string: '{}'", 
                line_count + 1, 
                line
            ));
        }

        converted_fes.push(conversion_output.unwrap());
        line_count += 1;
    }

    if line_count < k {
        log::error!("File {} contains only {} inputs, but {} inputs were requested", 
                    file_path, line_count, k);
        return Err(anyhow!("Insufficient inputs in file {}", file_path));
    }

    log::info!("Successfully read {} lines from {}", converted_fes.len(), file_path);
    Ok(converted_fes)
}

/// Alternative validation using LargeField if you want to use your existing field arithmetic
fn convert_string_to_large_field(input: &str) -> Option<LargeField> {
    let string_to_hex_string = |s: &str| -> String {
        let mut hex_string = String::new();
        for byte in s.as_bytes() {
            hex_string.push_str(&format!("{:02x}", byte));
        }
        hex_string
    };
    if let Ok(largefield_ele) = LargeField::from_hex(string_to_hex_string(input).as_str()){
        return Some(largefield_ele);
    }
    None
}