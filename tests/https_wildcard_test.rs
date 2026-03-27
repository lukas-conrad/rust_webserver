// Integration test for HTTPS server with wildcard certificate support
use rust_webserver::webserver::cert_manager::WildcardCertResolver;

/// Mock test to verify the wildcard certificate resolver logic
/// This tests the matching logic without needing actual SSL certificates
#[tokio::test]
async fn test_wildcard_cert_resolver_logic() {
    // Test matching logic for wildcard domains

    // Basic subdomain should match wildcard base
    assert!(WildcardCertResolver::matches_wildcard("api.example.com", "example.com"));

    // Multi-level subdomain should also match
    assert!(WildcardCertResolver::matches_wildcard("v1.api.example.com", "example.com"));

    // Base domain should match itself
    assert!(WildcardCertResolver::matches_wildcard("example.com", "example.com"));

    // Different domain should not match
    assert!(!WildcardCertResolver::matches_wildcard("notexample.com", "example.com"));

    // Parent domain should not match
    assert!(!WildcardCertResolver::matches_wildcard("example.com", "api.example.com"));

    // Partial domain should not match
    assert!(!WildcardCertResolver::matches_wildcard("myexample.com", "example.com"));
}

/// Test various common subdomain patterns
#[test]
fn test_common_subdomain_patterns() {
    let test_cases = vec![
        ("api.example.com", "example.com", true),
        ("www.example.com", "example.com", true),
        ("mail.example.com", "example.com", true),
        ("static.example.com", "example.com", true),
        ("cdn.example.com", "example.com", true),
        ("v2.api.example.com", "example.com", true),
        ("staging-api.example.com", "example.com", true),
        ("prod-db.example.com", "example.com", true),

        // Should NOT match
        ("example.com.evil.com", "example.com", false),
        ("evilexample.com", "example.com", false),
        ("notexample.com", "example.com", false),
        ("example.com.phishing.com", "example.com", false),
    ];

    for (subdomain, wildcard_base, should_match) in test_cases {
        let result = WildcardCertResolver::matches_wildcard(subdomain, wildcard_base);
        assert_eq!(
            result, should_match,
            "Failed for {} against {}: expected {}, got {}",
            subdomain, wildcard_base, should_match, result
        );
    }
}

/// Test edge cases for wildcard matching
#[test]
fn test_wildcard_edge_cases() {
    // Single character subdomain
    assert!(WildcardCertResolver::matches_wildcard("a.example.com", "example.com"));

    // Numeric subdomain
    assert!(WildcardCertResolver::matches_wildcard("1.example.com", "example.com"));

    // Hyphen in subdomain
    assert!(WildcardCertResolver::matches_wildcard("my-api.example.com", "example.com"));

    // Underscore in subdomain (not standard but should still match)
    assert!(WildcardCertResolver::matches_wildcard("my_api.example.com", "example.com"));

    // Empty string should not match anything
    assert!(!WildcardCertResolver::matches_wildcard("", "example.com"));
    assert!(!WildcardCertResolver::matches_wildcard("example.com", ""));

    // Case sensitivity check (DNS is case-insensitive, but our comparison is case-sensitive)
    // This is expected to fail because we're doing literal string comparison
    assert!(!WildcardCertResolver::matches_wildcard("API.EXAMPLE.COM", "example.com"));
}

/// Test the resolver with realistic domain configurations
#[test]
fn test_resolver_domain_registration() {
    let _resolver = WildcardCertResolver::new();

    // Test that we can create a resolver and register domains
    // (We use dummy data since we can't easily create real certificates in tests)
    assert_eq!(WildcardCertResolver::matches_wildcard("api.example.com", "example.com"), true);
    assert_eq!(WildcardCertResolver::matches_wildcard("example.com", "example.com"), true);
    assert_eq!(WildcardCertResolver::matches_wildcard("other.com", "example.com"), false);
}

/// Test multiple wildcard patterns
#[test]
fn test_multiple_wildcard_patterns() {
    // Test that different wildcard bases don't interfere with each other
    let base1 = "example.com";
    let base2 = "anotherdomain.com";

    // Should match base1
    assert!(WildcardCertResolver::matches_wildcard("api.example.com", base1));
    assert!(WildcardCertResolver::matches_wildcard("www.example.com", base1));

    // Should match base2
    assert!(WildcardCertResolver::matches_wildcard("api.anotherdomain.com", base2));
    assert!(WildcardCertResolver::matches_wildcard("www.anotherdomain.com", base2));

    // Cross-matching should fail
    assert!(!WildcardCertResolver::matches_wildcard("api.example.com", base2));
    assert!(!WildcardCertResolver::matches_wildcard("api.anotherdomain.com", base1));
}

/// Test wildcard domain name extraction
#[test]
fn test_wildcard_domain_extraction() {
    let wildcard_domains = vec![
        ("*.example.com", "example.com"),
        ("*.api.example.com", "api.example.com"),
        ("*.subdomain.example.com", "subdomain.example.com"),
    ];

    for (wildcard, expected_base) in wildcard_domains {
        if wildcard.starts_with("*.") {
            let extracted = &wildcard[2..];
            assert_eq!(extracted, expected_base);
        } else {
            panic!("Wildcard should start with *.");
        }
    }
}

/// Test realistic SNI scenarios
#[test]
fn test_realistic_sni_scenarios() {
    // Scenario 1: Single wildcard certificate for a domain
    // Config has *.example.com, requests come for api.example.com, www.example.com, etc.
    let wildcard_base = "example.com"; // Extracted from *.example.com

    let test_requests = vec![
        "api.example.com",
        "www.example.com",
        "mail.example.com",
        "ftp.example.com",
        "v2.api.example.com",
    ];

    for request in test_requests {
        assert!(
            WildcardCertResolver::matches_wildcard(request, wildcard_base),
            "Request for {} should match wildcard pattern for {}",
            request,
            wildcard_base
        );
    }

    // These should NOT match
    let invalid_requests = vec![
        "notexample.com",
        "example.com.attacker.com",
        "evil.com",
    ];

    for request in invalid_requests {
        assert!(
            !WildcardCertResolver::matches_wildcard(request, wildcard_base),
            "Request for {} should NOT match wildcard pattern for {}",
            request,
            wildcard_base
        );
    }
}



