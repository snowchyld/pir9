//! Release scoring module
//!
//! Applies release profiles (preferred words, must-contain, must-not-contain)
//! and custom format specifications to score and filter releases during
//! interactive search and RSS sync.

use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::core::datastore::models::{CustomFormatDbModel, ReleaseProfileDbModel};

/// A preferred word entry in a release profile.
/// Positive scores prefer the release, negative scores penalize it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferredWord {
    pub key: String,
    pub value: i32,
}

/// Result of scoring a release against all release profiles and custom formats.
#[derive(Debug, Clone, Default)]
pub struct ReleaseScore {
    /// Sum of all matched preferred word scores from release profiles
    pub preferred_word_score: i32,
    /// Sum of all matched custom format scores
    pub custom_format_score: i32,
    /// Total combined score
    pub total_score: i32,
    /// Whether the release should be rejected
    pub rejected: bool,
    /// Human-readable rejection reasons
    pub rejection_reasons: Vec<String>,
    /// IDs of custom formats that matched this release
    pub matched_custom_format_ids: Vec<i64>,
}

/// Score a release title against release profiles and custom formats.
///
/// Release profiles contribute:
/// - **Required terms**: if a profile has required terms and none match, the release is rejected
/// - **Ignored terms**: if any ignored term matches, the release is rejected
/// - **Preferred words**: each matching preferred word adds its score (positive or negative)
///
/// Custom formats contribute:
/// - Each format has specifications (regex patterns on the title). If ALL required specs match
///   (or any non-required spec matches when no required specs exist), the format matches.
/// - When a format matches, its score from the quality profile's `format_items` is looked up.
///
/// The `format_scores` parameter maps custom_format_id -> score (from the quality profile's
/// `format_items` field). If a custom format has no entry in this map, it contributes 0.
pub fn score_release(
    release_title: &str,
    release_profiles: &[ReleaseProfileDbModel],
    custom_formats: &[CustomFormatDbModel],
    format_scores: &std::collections::HashMap<i64, i32>,
) -> ReleaseScore {
    let mut result = ReleaseScore::default();

    // --- Release profiles ---
    for profile in release_profiles {
        if !profile.enabled {
            continue;
        }

        // Parse required terms
        let required: Vec<String> = serde_json::from_str(&profile.required).unwrap_or_default();
        // Parse ignored terms
        let ignored: Vec<String> = serde_json::from_str(&profile.ignored).unwrap_or_default();
        // Parse preferred words
        let preferred: Vec<PreferredWord> =
            serde_json::from_str(&profile.preferred).unwrap_or_default();

        // Must-contain check: if required terms exist, at least one must match
        if !required.is_empty() {
            let any_match = required
                .iter()
                .any(|term| title_contains_term(release_title, term));
            if !any_match {
                result.rejected = true;
                result.rejection_reasons.push(format!(
                    "Release profile '{}': none of the required terms matched",
                    profile.name
                ));
            }
        }

        // Must-not-contain check: if any ignored term matches, reject
        for term in &ignored {
            if title_contains_term(release_title, term) {
                result.rejected = true;
                result.rejection_reasons.push(format!(
                    "Release profile '{}': ignored term '{}' matched",
                    profile.name, term
                ));
            }
        }

        // Preferred words: sum up scores for matching terms
        for pw in &preferred {
            if title_contains_term(release_title, &pw.key) {
                result.preferred_word_score += pw.value;
            }
        }
    }

    // --- Custom formats ---
    for cf in custom_formats {
        if matches_custom_format(release_title, cf) {
            let score = format_scores.get(&cf.id).copied().unwrap_or(0);
            result.custom_format_score += score;
            result.matched_custom_format_ids.push(cf.id);
        }
    }

    result.total_score = result.preferred_word_score + result.custom_format_score;
    result
}

/// Check if a release title contains a term (case-insensitive substring match).
/// If the term looks like a regex (contains regex metacharacters), try to match as regex.
fn title_contains_term(title: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }

    // If the term contains regex-like characters, try regex first
    if term.contains('(')
        || term.contains('[')
        || term.contains('\\')
        || term.contains('|')
        || term.contains('^')
        || term.contains('$')
        || term.contains('+')
        || term.contains('?')
        || term.contains('{')
    {
        if let Ok(re) = RegexBuilder::new(term).case_insensitive(true).build() {
            return re.is_match(title);
        }
    }

    // Plain case-insensitive substring match
    title.to_lowercase().contains(&term.to_lowercase())
}

/// Check if a release title matches a custom format's specifications.
///
/// A custom format matches when:
/// - If there are required specs: ALL required specs must match
/// - If there are only non-required specs: at least one must match (OR logic)
/// - Each spec can be negated, which inverts the match result
fn matches_custom_format(title: &str, cf: &CustomFormatDbModel) -> bool {
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Spec {
        implementation: String,
        #[serde(default)]
        negate: bool,
        #[serde(default)]
        required: bool,
        #[serde(default)]
        fields: Vec<SpecField>,
    }

    #[derive(Debug, Deserialize)]
    struct SpecField {
        #[serde(default)]
        name: String,
        value: Option<serde_json::Value>,
    }

    let specs: Vec<Spec> = match serde_json::from_str(&cf.specifications) {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "Failed to parse custom format '{}' specifications: {}",
                cf.name, e
            );
            return false;
        }
    };

    if specs.is_empty() {
        return false;
    }

    let eval_spec = |spec: &Spec| -> bool {
        let fields: Vec<(&str, Option<&serde_json::Value>)> = spec
            .fields
            .iter()
            .map(|f| (f.name.as_str(), f.value.as_ref()))
            .collect();

        let raw_match = match spec.implementation.as_str() {
            "ReleaseTitleSpecification" | "ReleaseGroupSpecification" | "EditionSpecification" => {
                // These use a regex value field
                let pattern = fields
                    .iter()
                    .find(|(name, _)| *name == "value")
                    .and_then(|(_, v)| *v)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if pattern.is_empty() {
                    false
                } else {
                    match RegexBuilder::new(pattern).case_insensitive(true).build() {
                        Ok(re) => re.is_match(title),
                        Err(e) => {
                            warn!("Invalid regex in custom format '{}' spec: {}", cf.name, e);
                            false
                        }
                    }
                }
            }
            // These match on structured data, not title text.
            // A full implementation would inspect parsed quality/language/flags.
            // For title-based scoring they don't contribute.
            "SourceSpecification"
            | "ResolutionSpecification"
            | "QualityModifierSpecification"
            | "LanguageSpecification"
            | "IndexerFlagSpecification"
            | "SizeSpecification" => false,
            _ => false,
        };

        if spec.negate {
            !raw_match
        } else {
            raw_match
        }
    };

    let required_specs: Vec<&Spec> = specs.iter().filter(|s| s.required).collect();
    let optional_specs: Vec<&Spec> = specs.iter().filter(|s| !s.required).collect();

    // All required specs must match
    if !required_specs.is_empty() {
        let all_required_pass = required_specs.iter().all(|spec| eval_spec(spec));
        return all_required_pass;
    }

    // If only optional specs, at least one must match
    if !optional_specs.is_empty() {
        return optional_specs.iter().any(|spec| eval_spec(spec));
    }

    false
}

/// Check if a release's detected languages match the required language IDs.
/// Returns true if any of the release's languages matches any of the required ones.
/// If `required_language_ids` is empty, all releases pass (no filtering).
pub fn release_matches_language(release_title: &str, required_language_ids: &[i32]) -> bool {
    if required_language_ids.is_empty() {
        return true; // No language preference set — accept everything
    }

    let detected = crate::core::parser::detect_languages(release_title);
    detected
        .iter()
        .any(|lang| required_language_ids.contains(&lang.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_release_profile(
        name: &str,
        required: Vec<&str>,
        ignored: Vec<&str>,
        preferred: Vec<(&str, i32)>,
    ) -> ReleaseProfileDbModel {
        let required_json =
            serde_json::to_string(&required.into_iter().map(String::from).collect::<Vec<_>>())
                .unwrap();
        let ignored_json =
            serde_json::to_string(&ignored.into_iter().map(String::from).collect::<Vec<_>>())
                .unwrap();
        let preferred_json = serde_json::to_string(
            &preferred
                .into_iter()
                .map(|(k, v)| PreferredWord {
                    key: k.to_string(),
                    value: v,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();

        ReleaseProfileDbModel {
            id: 1,
            name: name.to_string(),
            enabled: true,
            required: required_json,
            ignored: ignored_json,
            preferred: preferred_json,
            include_preferred_when_renaming: false,
            indexer_id: 0,
            tags: "[]".to_string(),
        }
    }

    fn make_custom_format(id: i64, name: &str, specs_json: &str) -> CustomFormatDbModel {
        CustomFormatDbModel {
            id,
            name: name.to_string(),
            include_custom_format_when_renaming: false,
            specifications: specs_json.to_string(),
        }
    }

    #[test]
    fn test_preferred_word_positive_score() {
        let profiles = vec![make_release_profile(
            "Quality Words",
            vec![],
            vec![],
            vec![("REMUX", 100), ("x265", 50)],
        )];

        let score = score_release(
            "Movie.2024.1080p.REMUX.x265-GROUP",
            &profiles,
            &[],
            &HashMap::new(),
        );

        assert_eq!(score.preferred_word_score, 150);
        assert_eq!(score.total_score, 150);
        assert!(!score.rejected);
    }

    #[test]
    fn test_preferred_word_negative_score() {
        let profiles = vec![make_release_profile(
            "Avoid CAM",
            vec![],
            vec![],
            vec![("CAM", -100), ("TS", -50)],
        )];

        let score = score_release("Movie.2024.CAM.x264-JUNK", &profiles, &[], &HashMap::new());

        assert_eq!(score.preferred_word_score, -100);
        assert_eq!(score.total_score, -100);
        assert!(!score.rejected);
    }

    #[test]
    fn test_preferred_word_mixed_scores() {
        let profiles = vec![make_release_profile(
            "Mixed",
            vec![],
            vec![],
            vec![("1080p", 10), ("x265", 20), ("YTS", -50)],
        )];

        let score = score_release("Movie.2024.1080p.x265-YTS", &profiles, &[], &HashMap::new());

        assert_eq!(score.preferred_word_score, -20); // 10 + 20 - 50
        assert!(!score.rejected);
    }

    #[test]
    fn test_preferred_word_case_insensitive() {
        let profiles = vec![make_release_profile(
            "Case Test",
            vec![],
            vec![],
            vec![("remux", 100)],
        )];

        let score = score_release(
            "Movie.2024.1080p.REMUX.DTS-HD-GROUP",
            &profiles,
            &[],
            &HashMap::new(),
        );

        assert_eq!(score.preferred_word_score, 100);
    }

    #[test]
    fn test_must_contain_passes() {
        let profiles = vec![make_release_profile(
            "Require 1080p",
            vec!["1080p"],
            vec![],
            vec![],
        )];

        let score = score_release(
            "Movie.2024.1080p.BluRay-GROUP",
            &profiles,
            &[],
            &HashMap::new(),
        );

        assert!(!score.rejected);
        assert!(score.rejection_reasons.is_empty());
    }

    #[test]
    fn test_must_contain_rejects() {
        let profiles = vec![make_release_profile(
            "Require 1080p",
            vec!["1080p"],
            vec![],
            vec![],
        )];

        let score = score_release(
            "Movie.2024.720p.BluRay-GROUP",
            &profiles,
            &[],
            &HashMap::new(),
        );

        assert!(score.rejected);
        assert_eq!(score.rejection_reasons.len(), 1);
        assert!(score.rejection_reasons[0].contains("required terms"));
    }

    #[test]
    fn test_must_contain_any_of_multiple() {
        // If multiple required terms exist, ANY one matching is sufficient
        let profiles = vec![make_release_profile(
            "Require HD",
            vec!["1080p", "2160p"],
            vec![],
            vec![],
        )];

        let score = score_release(
            "Movie.2024.2160p.BluRay-GROUP",
            &profiles,
            &[],
            &HashMap::new(),
        );

        assert!(!score.rejected);
    }

    #[test]
    fn test_must_not_contain_passes() {
        let profiles = vec![make_release_profile(
            "No CAM",
            vec![],
            vec!["CAM", "TS", "HDTS"],
            vec![],
        )];

        let score = score_release(
            "Movie.2024.1080p.BluRay-GROUP",
            &profiles,
            &[],
            &HashMap::new(),
        );

        assert!(!score.rejected);
    }

    #[test]
    fn test_must_not_contain_rejects() {
        let profiles = vec![make_release_profile(
            "No CAM",
            vec![],
            vec!["CAM", "TS"],
            vec![],
        )];

        let score = score_release(
            "Movie.2024.HDCAM.x264-JUNK",
            &profiles,
            &[],
            &HashMap::new(),
        );

        assert!(score.rejected);
        assert!(score.rejection_reasons[0].contains("ignored term 'CAM'"));
    }

    #[test]
    fn test_disabled_profile_skipped() {
        let mut profile = make_release_profile("Disabled", vec!["IMPOSSIBLE_TERM"], vec![], vec![]);
        profile.enabled = false;

        let score = score_release(
            "Movie.2024.1080p.BluRay-GROUP",
            &[profile],
            &[],
            &HashMap::new(),
        );

        assert!(!score.rejected);
    }

    #[test]
    fn test_custom_format_regex_matching() {
        let cf = make_custom_format(
            1,
            "x265",
            r#"[{"implementation": "ReleaseTitleSpecification", "negate": false, "required": false, "fields": [{"name": "value", "value": "x265|h\\.?265|hevc"}]}]"#,
        );

        let mut format_scores = HashMap::new();
        format_scores.insert(1, 25);

        let score = score_release(
            "Movie.2024.1080p.BluRay.x265-GROUP",
            &[],
            &[cf],
            &format_scores,
        );

        assert_eq!(score.custom_format_score, 25);
        assert!(score.matched_custom_format_ids.contains(&1));
    }

    #[test]
    fn test_custom_format_no_match() {
        let cf = make_custom_format(
            1,
            "x265",
            r#"[{"implementation": "ReleaseTitleSpecification", "negate": false, "required": false, "fields": [{"name": "value", "value": "x265"}]}]"#,
        );

        let mut format_scores = HashMap::new();
        format_scores.insert(1, 25);

        let score = score_release(
            "Movie.2024.1080p.BluRay.x264-GROUP",
            &[],
            &[cf],
            &format_scores,
        );

        assert_eq!(score.custom_format_score, 0);
        assert!(score.matched_custom_format_ids.is_empty());
    }

    #[test]
    fn test_custom_format_negated_spec() {
        // Negated spec: matches when the regex does NOT match
        let cf = make_custom_format(
            2,
            "Not x265",
            r#"[{"implementation": "ReleaseTitleSpecification", "negate": true, "required": false, "fields": [{"name": "value", "value": "x265"}]}]"#,
        );

        let mut format_scores = HashMap::new();
        format_scores.insert(2, 10);

        // This title does NOT contain x265, so negated spec matches
        let score = score_release(
            "Movie.2024.1080p.BluRay.x264-GROUP",
            &[],
            &[cf],
            &format_scores,
        );

        assert_eq!(score.custom_format_score, 10);
        assert!(score.matched_custom_format_ids.contains(&2));
    }

    #[test]
    fn test_custom_format_required_spec() {
        // Two specs: first is required, second is optional
        let cf = make_custom_format(
            3,
            "BluRay x265",
            r#"[
                {"implementation": "ReleaseTitleSpecification", "negate": false, "required": true, "fields": [{"name": "value", "value": "blu.?ray"}]},
                {"implementation": "ReleaseTitleSpecification", "negate": false, "required": false, "fields": [{"name": "value", "value": "x265"}]}
            ]"#,
        );

        let mut format_scores = HashMap::new();
        format_scores.insert(3, 50);

        // Has BluRay (required) -- should match
        let score = score_release(
            "Movie.2024.1080p.BluRay.x264-GROUP",
            &[],
            &[cf.clone()],
            &format_scores,
        );
        assert_eq!(score.custom_format_score, 50);

        // Missing BluRay (required) -- should NOT match
        let score = score_release(
            "Movie.2024.1080p.WEB.x265-GROUP",
            &[],
            &[cf],
            &format_scores,
        );
        assert_eq!(score.custom_format_score, 0);
    }

    #[test]
    fn test_combined_scoring() {
        let profiles = vec![make_release_profile(
            "Prefer x265",
            vec![],
            vec![],
            vec![("x265", 30), ("REMUX", 50)],
        )];

        let cf = make_custom_format(
            1,
            "BluRay",
            r#"[{"implementation": "ReleaseTitleSpecification", "negate": false, "required": false, "fields": [{"name": "value", "value": "blu.?ray"}]}]"#,
        );

        let mut format_scores = HashMap::new();
        format_scores.insert(1, 20);

        let score = score_release(
            "Movie.2024.1080p.BluRay.REMUX.x265-GROUP",
            &profiles,
            &[cf],
            &format_scores,
        );

        assert_eq!(score.preferred_word_score, 80); // 30 + 50
        assert_eq!(score.custom_format_score, 20);
        assert_eq!(score.total_score, 100);
        assert!(!score.rejected);
    }

    #[test]
    fn test_empty_profiles_and_formats() {
        let score = score_release("Movie.2024.1080p.BluRay-GROUP", &[], &[], &HashMap::new());

        assert_eq!(score.total_score, 0);
        assert!(!score.rejected);
        assert!(score.rejection_reasons.is_empty());
    }

    #[test]
    fn test_custom_format_empty_specs() {
        let cf = make_custom_format(1, "Empty", "[]");
        let mut format_scores = HashMap::new();
        format_scores.insert(1, 100);

        let score = score_release("Movie.2024.1080p.BluRay-GROUP", &[], &[cf], &format_scores);

        assert_eq!(score.custom_format_score, 0);
    }

    #[test]
    fn test_regex_preferred_word() {
        let profiles = vec![make_release_profile(
            "Regex Prefs",
            vec![],
            vec![],
            vec![("(x|h)\\.?265", 50)],
        )];

        let score = score_release(
            "Movie.2024.1080p.BluRay.h.265-GROUP",
            &profiles,
            &[],
            &HashMap::new(),
        );

        assert_eq!(score.preferred_word_score, 50);
    }

    #[test]
    fn test_multiple_profiles_combined() {
        let profiles = vec![
            make_release_profile("Profile A", vec![], vec![], vec![("1080p", 10)]),
            make_release_profile("Profile B", vec![], vec![], vec![("BluRay", 20)]),
        ];

        let score = score_release(
            "Movie.2024.1080p.BluRay-GROUP",
            &profiles,
            &[],
            &HashMap::new(),
        );

        assert_eq!(score.preferred_word_score, 30);
    }

    #[test]
    fn test_title_contains_term_empty() {
        assert!(!title_contains_term("some title", ""));
        assert!(!title_contains_term("", "term"));
    }
}
