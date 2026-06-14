use xechat::services::embedder::{find_sentence_boundary, find_role_label_boundary, find_label_overlap_boundary, normalize_vector};

// ── find_sentence_boundary ──────────────────────────────────────

#[test]
fn test_find_sentence_boundary_chinese_period() {
    let text = "这是第一句\u{3002}这是第二句\u{3002}这是第三句";
    // Search for boundary after target position
    let result = find_sentence_boundary(text, 0, text.len());
    assert!(result.is_some(), "Should find a sentence boundary");
    let pos = result.unwrap();
    assert!(pos > 0, "Boundary should be after start");
}

#[test]
fn test_find_sentence_boundary_english_period() {
    let text = "First sentence. Second sentence. Third sentence";
    let result = find_sentence_boundary(text, 0, text.len());
    assert!(result.is_some());
    let pos = result.unwrap();
    assert!(text[..pos].ends_with('.') || text[..pos].ends_with(". "));
}

#[test]
fn test_find_sentence_boundary_exclamation() {
    let text = "Hello! World";
    let result = find_sentence_boundary(text, 0, text.len());
    assert!(result.is_some());
}

#[test]
fn test_find_sentence_boundary_question() {
    let text = "How are you? Fine.";
    let result = find_sentence_boundary(text, 0, text.len());
    assert!(result.is_some());
}

#[test]
fn test_find_sentence_boundary_no_ending() {
    let text = "no ending punctuation here";
    let result = find_sentence_boundary(text, 0, text.len());
    assert!(result.is_none(), "Should return None when no sentence ending found");
}

#[test]
fn test_find_sentence_boundary_empty() {
    let text = "";
    let result = find_sentence_boundary(text, 0, 0);
    assert!(result.is_none());
}

#[test]
fn test_find_sentence_boundary_with_range() {
    let text = "A. B. C. D.";
    // Only search in the middle portion
    let result = find_sentence_boundary(text, 2, 8);
    assert!(result.is_some());
}

// ── find_role_label_boundary ────────────────────────────────────

#[test]
fn test_find_role_label_boundary_user_label() {
    let text = "一些文本\n用户：你好";
    let result = find_role_label_boundary(text, 0, text.len());
    assert!(result.is_some(), "Should find boundary before 用户：");
    let pos = result.unwrap();
    assert!(text[pos..].starts_with("用户："));
}

#[test]
fn test_find_role_label_boundary_assistant_label() {
    let text = "一些文本\n助手：你好";
    let result = find_role_label_boundary(text, 0, text.len());
    assert!(result.is_some(), "Should find boundary before 助手：");
}

#[test]
fn test_find_role_label_boundary_no_label() {
    let text = "普通文本\n没有标签";
    let result = find_role_label_boundary(text, 0, text.len());
    assert!(result.is_none(), "Should return None when no role label found");
}

#[test]
fn test_find_role_label_boundary_no_newline() {
    let text = "用户：你好";
    let result = find_role_label_boundary(text, 0, text.len());
    assert!(result.is_none(), "Should return None when no newline found");
}

// ── find_label_overlap_boundary ─────────────────────────────────

#[test]
fn test_find_label_overlap_boundary_cut_inside_label() {
    let text = "一些文本\n用户：你好\n更多文本";
    // Cut point inside "用户："
    let label = "用户：";
    let label_start = text.find(label).unwrap();
    let cut_point = label_start + 3; // Inside the label
    let result = find_label_overlap_boundary(text, cut_point, label);
    assert!(result.is_some(), "Should detect cut inside label");
    let boundary = result.unwrap();
    assert!(boundary <= label_start, "Boundary should be at or before the label");
}

#[test]
fn test_find_label_overlap_boundary_cut_outside_label() {
    let text = "一些文本\n用户：你好\n更多文本";
    let label = "用户：";
    let cut_point = 3; // Before the label
    let result = find_label_overlap_boundary(text, cut_point, label);
    assert!(result.is_none(), "Should return None when cut is outside label");
}

#[test]
fn test_find_label_overlap_boundary_cut_after_label() {
    let text = "用户：你好";
    let label = "用户：";
    let label_end = text.find(label).unwrap() + label.len();
    let cut_point = label_end + 1; // After the label
    let result = find_label_overlap_boundary(text, cut_point, label);
    assert!(result.is_none(), "Should return None when cut is after label");
}

// ── normalize_vector ────────────────────────────────────────────

#[test]
fn test_normalize_vector_unit() {
    let mut vec = vec![1.0, 0.0, 0.0];
    normalize_vector(&mut vec);
    assert!((vec[0] - 1.0).abs() < 1e-6);
    assert!(vec[1].abs() < 1e-6);
    assert!(vec[2].abs() < 1e-6);
}

#[test]
fn test_normalize_vector_general() {
    let mut vec = vec![3.0, 4.0];
    normalize_vector(&mut vec);
    let norm = (vec[0] * vec[0] + vec[1] * vec[1]).sqrt();
    assert!((norm - 1.0).abs() < 1e-6, "L2 norm should be 1, got {}", norm);
}

#[test]
fn test_normalize_vector_zero() {
    let mut vec = vec![0.0, 0.0, 0.0];
    normalize_vector(&mut vec);
    assert_eq!(vec, vec![0.0, 0.0, 0.0], "Zero vector should remain zero");
}

#[test]
fn test_normalize_vector_negative() {
    let mut vec = vec![-3.0, -4.0];
    normalize_vector(&mut vec);
    let norm = (vec[0] * vec[0] + vec[1] * vec[1]).sqrt();
    assert!((norm - 1.0).abs() < 1e-6);
}

#[test]
fn test_normalize_vector_empty() {
    let mut vec: Vec<f32> = vec![];
    normalize_vector(&mut vec);
    assert!(vec.is_empty());
}
