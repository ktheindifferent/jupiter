use std::fmt;

/// Input sanitization module for database operations
/// Provides validation and sanitization functions to prevent SQL injection
pub struct InputSanitizer;

impl InputSanitizer {
    /// Validates an OID string to ensure it's safe for database operations
    /// Returns true if the OID is valid, false otherwise
    pub fn validate_oid(oid: &str) -> bool {
        // OID should only contain alphanumeric characters, underscores, and hyphens
        // Maximum length of 255 characters
        if oid.is_empty() || oid.len() > 255 {
            return false;
        }
        
        oid.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }
    
    /// Validates a column name for ORDER BY clauses
    /// Only allows whitelisted column names to prevent injection
    pub fn validate_order_column(column: &str, valid_columns: &[&str]) -> bool {
        valid_columns.contains(&column)
    }
    
    /// Validates limit and offset values
    /// Ensures they are within reasonable bounds
    pub fn validate_limit(limit: usize) -> Result<usize, ValidationError> {
        const MAX_LIMIT: usize = 1000;
        
        if limit == 0 {
            return Err(ValidationError::InvalidLimit("Limit must be greater than 0".to_string()));
        }
        
        if limit > MAX_LIMIT {
            return Err(ValidationError::InvalidLimit(format!("Limit cannot exceed {}", MAX_LIMIT)));
        }
        
        Ok(limit)
    }
    
    pub fn validate_offset(offset: usize) -> Result<usize, ValidationError> {
        const MAX_OFFSET: usize = 100000;
        
        if offset > MAX_OFFSET {
            return Err(ValidationError::InvalidOffset(format!("Offset cannot exceed {}", MAX_OFFSET)));
        }
        
        Ok(offset)
    }
    
    /// Sanitizes a string by escaping potentially dangerous characters
    /// This is a last resort - parameterized queries should be used instead
    pub fn escape_string(input: &str) -> String {
        input
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('"', "\\\"")
            .replace('\0', "\\0")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\x1a', "\\Z")
    }
    
    /// Validates that a string doesn't contain SQL keywords that could indicate injection
    pub fn check_for_sql_keywords(input: &str) -> bool {
        let dangerous_keywords = vec![
            "DROP", "DELETE", "INSERT", "UPDATE", "EXEC", "EXECUTE",
            "UNION", "SELECT", "CREATE", "ALTER", "TRUNCATE",
            "--", "/*", "*/", "xp_", "sp_", "0x"
        ];
        
        let input_upper = input.to_uppercase();
        
        !dangerous_keywords.iter().any(|keyword| input_upper.contains(keyword))
    }
    
    /// Sanitizes numeric input to ensure it's a valid number
    pub fn sanitize_numeric(input: &str) -> Result<i64, ValidationError> {
        input.parse::<i64>()
            .map_err(|_| ValidationError::InvalidNumeric(format!("'{}' is not a valid number", input)))
    }
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    InvalidOid(String),
    InvalidColumn(String),
    InvalidLimit(String),
    InvalidOffset(String),
    InvalidNumeric(String),
    SqlInjectionDetected(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ValidationError::InvalidOid(msg) => write!(f, "Invalid OID: {}", msg),
            ValidationError::InvalidColumn(msg) => write!(f, "Invalid column: {}", msg),
            ValidationError::InvalidLimit(msg) => write!(f, "Invalid limit: {}", msg),
            ValidationError::InvalidOffset(msg) => write!(f, "Invalid offset: {}", msg),
            ValidationError::InvalidNumeric(msg) => write!(f, "Invalid numeric value: {}", msg),
            ValidationError::SqlInjectionDetected(msg) => write!(f, "SQL injection attempt detected: {}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Middleware for validating all database inputs
pub struct DatabaseInputValidator;

impl DatabaseInputValidator {
    /// Validates all inputs before they're used in database queries
    pub fn validate_query_params(
        oid: Option<&str>,
        order_column: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
        valid_columns: &[&str],
    ) -> Result<(), ValidationError> {
        // Validate OID if provided
        if let Some(oid_val) = oid {
            if !InputSanitizer::validate_oid(oid_val) {
                return Err(ValidationError::InvalidOid(format!("Invalid OID format: {}", oid_val)));
            }
            
            if !InputSanitizer::check_for_sql_keywords(oid_val) {
                return Err(ValidationError::SqlInjectionDetected(format!("Suspicious input detected in OID: {}", oid_val)));
            }
        }
        
        // Validate order column if provided
        if let Some(col) = order_column {
            if !InputSanitizer::validate_order_column(col, valid_columns) {
                return Err(ValidationError::InvalidColumn(format!("Invalid order column: {}", col)));
            }
        }
        
        // Validate limit if provided
        if let Some(limit_val) = limit {
            InputSanitizer::validate_limit(limit_val)?;
        }
        
        // Validate offset if provided
        if let Some(offset_val) = offset {
            InputSanitizer::validate_offset(offset_val)?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_oid() {
        // Valid OIDs
        assert!(InputSanitizer::validate_oid("abc123"));
        assert!(InputSanitizer::validate_oid("test_oid_123"));
        assert!(InputSanitizer::validate_oid("uuid-1234-5678"));
        
        // Invalid OIDs
        assert!(!InputSanitizer::validate_oid("")); // Empty
        assert!(!InputSanitizer::validate_oid("a".repeat(256).as_str())); // Too long
        assert!(!InputSanitizer::validate_oid("test'; DROP TABLE--")); // SQL injection
        assert!(!InputSanitizer::validate_oid("test/*comment*/")); // SQL comment
    }
    
    #[test]
    fn test_check_for_sql_keywords() {
        // Clean inputs
        assert!(InputSanitizer::check_for_sql_keywords("normal_text"));
        assert!(InputSanitizer::check_for_sql_keywords("user123"));
        
        // Malicious inputs
        assert!(!InputSanitizer::check_for_sql_keywords("DROP TABLE users"));
        assert!(!InputSanitizer::check_for_sql_keywords("'; DELETE FROM--"));
        assert!(!InputSanitizer::check_for_sql_keywords("UNION SELECT * FROM"));
    }
    
    #[test]
    fn test_validate_limit() {
        assert!(InputSanitizer::validate_limit(10).is_ok());
        assert!(InputSanitizer::validate_limit(100).is_ok());
        assert!(InputSanitizer::validate_limit(1000).is_ok());
        
        assert!(InputSanitizer::validate_limit(0).is_err());
        assert!(InputSanitizer::validate_limit(1001).is_err());
    }
    
    #[test]
    fn test_escape_string() {
        assert_eq!(InputSanitizer::escape_string("normal"), "normal");
        assert_eq!(InputSanitizer::escape_string("test'quote"), "test\\'quote");
        assert_eq!(InputSanitizer::escape_string("test\"quote"), "test\\\"quote");
        assert_eq!(InputSanitizer::escape_string("test\nline"), "test\\nline");
    }
}