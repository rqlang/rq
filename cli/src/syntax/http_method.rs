#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
}
impl HttpMethod {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Some(Self::GET),
            "POST" => Some(Self::POST),
            "PUT" => Some(Self::PUT),
            "DELETE" => Some(Self::DELETE),
            "PATCH" => Some(Self::PATCH),
            "HEAD" => Some(Self::HEAD),
            "OPTIONS" => Some(Self::OPTIONS),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GET => "GET",
            Self::POST => "POST",
            Self::PUT => "PUT",
            Self::DELETE => "DELETE",
            Self::PATCH => "PATCH",
            Self::HEAD => "HEAD",
            Self::OPTIONS => "OPTIONS",
        }
    }

    pub fn to_reqwest_method(&self) -> reqwest::Method {
        match self {
            Self::GET => reqwest::Method::GET,
            Self::POST => reqwest::Method::POST,
            Self::PUT => reqwest::Method::PUT,
            Self::DELETE => reqwest::Method::DELETE,
            Self::PATCH => reqwest::Method::PATCH,
            Self::HEAD => reqwest::Method::HEAD,
            Self::OPTIONS => reqwest::Method::OPTIONS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        assert_eq!(HttpMethod::from_str("GET"), Some(HttpMethod::GET));
        assert_eq!(HttpMethod::from_str("get"), Some(HttpMethod::GET));
        assert_eq!(HttpMethod::from_str("POST"), Some(HttpMethod::POST));
        assert_eq!(HttpMethod::from_str("post"), Some(HttpMethod::POST));
        assert_eq!(HttpMethod::from_str("PUT"), Some(HttpMethod::PUT));
        assert_eq!(HttpMethod::from_str("DELETE"), Some(HttpMethod::DELETE));
        assert_eq!(HttpMethod::from_str("PATCH"), Some(HttpMethod::PATCH));
        assert_eq!(HttpMethod::from_str("HEAD"), Some(HttpMethod::HEAD));
        assert_eq!(HttpMethod::from_str("OPTIONS"), Some(HttpMethod::OPTIONS));

        assert_eq!(HttpMethod::from_str("INVALID"), None);
        assert_eq!(HttpMethod::from_str(""), None);
    }

    #[test]
    fn test_as_str() {
        assert_eq!(HttpMethod::GET.as_str(), "GET");
        assert_eq!(HttpMethod::POST.as_str(), "POST");
        assert_eq!(HttpMethod::PUT.as_str(), "PUT");
        assert_eq!(HttpMethod::DELETE.as_str(), "DELETE");
        assert_eq!(HttpMethod::PATCH.as_str(), "PATCH");
        assert_eq!(HttpMethod::HEAD.as_str(), "HEAD");
        assert_eq!(HttpMethod::OPTIONS.as_str(), "OPTIONS");
    }

    #[test]
    fn test_to_reqwest_method() {
        assert_eq!(HttpMethod::GET.to_reqwest_method(), reqwest::Method::GET);
        assert_eq!(HttpMethod::POST.to_reqwest_method(), reqwest::Method::POST);
        assert_eq!(HttpMethod::PUT.to_reqwest_method(), reqwest::Method::PUT);
        assert_eq!(
            HttpMethod::DELETE.to_reqwest_method(),
            reqwest::Method::DELETE
        );
        assert_eq!(
            HttpMethod::PATCH.to_reqwest_method(),
            reqwest::Method::PATCH
        );
        assert_eq!(HttpMethod::HEAD.to_reqwest_method(), reqwest::Method::HEAD);
        assert_eq!(
            HttpMethod::OPTIONS.to_reqwest_method(),
            reqwest::Method::OPTIONS
        );
    }
}
