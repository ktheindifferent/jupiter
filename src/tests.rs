#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_empty_array_safe_access() {
        let empty_vec: Vec<i32> = vec![];
        
        // Test .first() method - should return None for empty vector
        assert_eq!(empty_vec.first(), None);
        
        // Test .get(0) method - should return None for empty vector  
        assert_eq!(empty_vec.get(0), None);
        
        // Test that we can safely handle empty collections
        match empty_vec.first() {
            Some(_) => panic!("Should not have a first element"),
            None => {} // Expected behavior
        }
    }
    
    #[test]
    fn test_single_element_safe_access() {
        let single_vec = vec![42];
        
        // Test .first() method - should return Some(&42)
        assert_eq!(single_vec.first(), Some(&42));
        
        // Test .get(0) method - should return Some(&42)
        assert_eq!(single_vec.get(0), Some(&42));
        
        // Test out of bounds access - should return None
        assert_eq!(single_vec.get(1), None);
    }
    
    #[test]
    fn test_multiple_elements_safe_access() {
        let multi_vec = vec![1, 2, 3, 4, 5];
        
        // Test .first() method
        assert_eq!(multi_vec.first(), Some(&1));
        
        // Test .get() for various indices
        assert_eq!(multi_vec.get(0), Some(&1));
        assert_eq!(multi_vec.get(2), Some(&3));
        assert_eq!(multi_vec.get(4), Some(&5));
        
        // Test out of bounds access
        assert_eq!(multi_vec.get(5), None);
        assert_eq!(multi_vec.get(100), None);
    }
    
    #[test]
    fn test_is_empty_check() {
        let empty: Vec<String> = vec![];
        let not_empty = vec!["data".to_string()];
        
        assert!(empty.is_empty());
        assert!(!not_empty.is_empty());
        
        // Safe pattern using is_empty check
        if !not_empty.is_empty() {
            // Safe to access first element
            let _ = &not_empty[0];
        }
    }
}