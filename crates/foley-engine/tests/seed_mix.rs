use foley_engine::hit_seed;

#[test]
fn hit_seed_is_deterministic_and_field_sensitive() {
    assert_eq!(hit_seed(1, 60, 0.5, 0), hit_seed(1, 60, 0.5, 0));
    assert_ne!(hit_seed(1, 60, 0.5, 0), hit_seed(1, 60, 0.5, 1));
    assert_ne!(hit_seed(1, 60, 0.5, 0), hit_seed(1, 61, 0.5, 0));
    assert_ne!(hit_seed(1, 60, 0.5, 0), hit_seed(1, 60, 0.75, 0));
}
