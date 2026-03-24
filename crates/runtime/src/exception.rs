//! Java exception support.
//!
//! Java `throw` / `try-catch-finally` is lowered to Rust panics plus
//! `std::panic::catch_unwind`.  Exception values are encoded in the panic
//! payload as the string `"JException:{ClassName}:{message}"` so that the
//! catch handler can decode and match them.

use crate::JString;

/// A Java exception value carried through `catch_unwind`.
///
/// Exceptions are encoded in panics as `"JException:{ClassName}:{message}"`.
#[derive(Debug, Clone)]
pub struct JException {
    class_name: String,
    message: JString,
}

impl JException {
    /// Create a new exception with the given class name and message.
    pub fn new(class_name: impl Into<String>, message: impl Into<JString>) -> Self {
        Self {
            class_name: class_name.into(),
            message: message.into(),
        }
    }

    /// Try to decode a `JException` from a `catch_unwind` panic payload.
    pub fn from_panic_payload(payload: &Box<dyn std::any::Any + Send>) -> Option<Self> {
        let msg: &str = if let Some(s) = payload.downcast_ref::<String>() {
            s.as_str()
        } else if let Some(s) = payload.downcast_ref::<&str>() {
            s
        } else {
            return None;
        };
        Self::from_panic_msg(msg)
    }

    /// Try to decode a `JException` from a panic message string.
    pub fn from_panic_msg(msg: &str) -> Option<Self> {
        let rest = msg.strip_prefix("JException:")?;
        let (class_name, message) = rest.split_once(':').unwrap_or((rest, ""));
        Some(Self {
            class_name: class_name.to_string(),
            message: JString::from(message),
        })
    }

    /// Returns true if this exception is an instance of `check` (simple
    /// name-based hierarchy for the most common Java exception types).
    pub fn is_instance_of(&self, check: &str) -> bool {
        if self.class_name == check {
            return true;
        }
        // Base types that catch everything
        if matches!(check, "Throwable" | "Exception") {
            return true;
        }
        // Known RuntimeException subclasses
        let is_runtime = matches!(
            self.class_name.as_str(),
            "RuntimeException"
                | "ArithmeticException"
                | "NullPointerException"
                | "ClassCastException"
                | "IllegalArgumentException"
                | "IllegalStateException"
                | "IndexOutOfBoundsException"
                | "ArrayIndexOutOfBoundsException"
                | "StringIndexOutOfBoundsException"
                | "NumberFormatException"
                | "UnsupportedOperationException"
                | "StackOverflowError"
                | "ConcurrentModificationException"
        );
        if check == "RuntimeException" && is_runtime {
            return true;
        }
        // IndexOutOfBoundsException hierarchy
        if check == "IndexOutOfBoundsException"
            && matches!(
                self.class_name.as_str(),
                "ArrayIndexOutOfBoundsException" | "StringIndexOutOfBoundsException"
            )
        {
            return true;
        }
        false
    }

    /// Returns the Java class name of this exception.
    pub fn get_class_name(&self) -> &str {
        &self.class_name
    }

    /// Returns the exception message (mirrors `Throwable.getMessage()`).
    #[allow(non_snake_case)]
    pub fn getMessage(&self) -> JString {
        self.message.clone()
    }

    /// Encode to the panic string format `"JException:{ClassName}:{message}"`.
    pub fn to_panic_string(&self) -> String {
        format!("JException:{}:{}", self.class_name, self.message)
    }
}

impl std::fmt::Display for JException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.class_name, self.message)
    }
}

impl std::error::Error for JException {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_panic_string() {
        let ex = JException::new("ArithmeticException", "divide by zero");
        let s = ex.to_panic_string();
        let back = JException::from_panic_msg(&s).unwrap();
        assert_eq!(back.class_name, "ArithmeticException");
        assert_eq!(back.getMessage().to_string(), "divide by zero");
    }

    #[test]
    fn is_instance_of_hierarchy() {
        let ex = JException::new("ArithmeticException", "");
        assert!(ex.is_instance_of("ArithmeticException"));
        assert!(ex.is_instance_of("RuntimeException"));
        assert!(ex.is_instance_of("Exception"));
        assert!(ex.is_instance_of("Throwable"));
        assert!(!ex.is_instance_of("IOException"));
    }

    #[test]
    fn from_panic_payload_string() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(String::from(
            "JException:IllegalArgumentException:bad input",
        ));
        let ex = JException::from_panic_payload(&payload).unwrap();
        assert_eq!(ex.class_name, "IllegalArgumentException");
        assert!(ex.is_instance_of("RuntimeException"));
    }
}
