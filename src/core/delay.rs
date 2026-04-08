//! Delay Profile logic
//! Determines whether a release should be delayed before grabbing,
//! based on configured delay profiles, protocol preferences, and tag matching.

use chrono::{DateTime, Duration, Utc};
use tracing::{debug, info};

use crate::core::datastore::models::DelayProfileDbModel;
use crate::core::indexers::Protocol;

/// Result of checking whether a release should be delayed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelayDecision {
    /// Grab immediately — no delay applies
    GrabNow,
    /// Delay until the specified time
    DelayUntil(DateTime<Utc>),
}

/// Find the most specific delay profile that applies to a set of entity tags.
///
/// Profiles are checked in `order` (ascending). A profile matches if:
/// - It has no tags (applies to everything), OR
/// - At least one of its tags matches an entity tag
///
/// The first matching profile with tags wins. If none match, the first
/// profile with no tags (the "default" profile) wins.
pub fn find_matching_profile<'a>(
    profiles: &'a [DelayProfileDbModel],
    entity_tags: &[i64],
) -> Option<&'a DelayProfileDbModel> {
    // Profiles are already sorted by order from the repository
    let mut default_profile: Option<&DelayProfileDbModel> = None;

    for profile in profiles {
        let profile_tags: Vec<i32> = serde_json::from_str(&profile.tags).unwrap_or_default();

        if profile_tags.is_empty() {
            // Tag-less profile is the default fallback
            if default_profile.is_none() {
                default_profile = Some(profile);
            }
            continue;
        }

        // Check if any profile tag matches any entity tag
        let has_match = profile_tags
            .iter()
            .any(|&pt| entity_tags.contains(&(pt as i64)));

        if has_match {
            return Some(profile);
        }
    }

    default_profile
}

/// Check whether a release should be delayed based on the matching delay profile.
///
/// Arguments:
/// - `profile`: The matching delay profile
/// - `protocol`: The protocol of the release (usenet or torrent)
/// - `air_date_utc`: When the episode aired (or movie release date) — used as the
///   reference point for the delay window. If `None`, grab immediately.
///
/// Returns a `DelayDecision` indicating whether to grab now or delay.
pub fn check_delay(
    profile: &DelayProfileDbModel,
    protocol: Protocol,
    air_date_utc: Option<DateTime<Utc>>,
) -> DelayDecision {
    let air_date = match air_date_utc {
        Some(d) => d,
        None => {
            // No air date means we can't calculate delay — grab immediately
            debug!("No air date, skipping delay check");
            return DelayDecision::GrabNow;
        }
    };

    // Get the delay in minutes for this protocol
    let delay_minutes = match protocol {
        Protocol::Usenet => {
            if !profile.enable_usenet {
                return DelayDecision::GrabNow;
            }
            profile.usenet_delay
        }
        Protocol::Torrent => {
            if !profile.enable_torrent {
                return DelayDecision::GrabNow;
            }
            profile.torrent_delay
        }
        Protocol::Unknown => return DelayDecision::GrabNow,
    };

    // No delay configured for this protocol
    if delay_minutes <= 0 {
        return DelayDecision::GrabNow;
    }

    // Check if this is the preferred protocol — preferred protocol has no delay
    let preferred = match profile.preferred_protocol {
        1 => Protocol::Usenet,
        2 => Protocol::Torrent,
        _ => Protocol::Unknown,
    };

    if protocol == preferred {
        debug!("Release uses preferred protocol {:?}, no delay", protocol);
        return DelayDecision::GrabNow;
    }

    // Calculate when the delay expires: air_date + delay_minutes
    let delay_until = air_date + Duration::minutes(delay_minutes as i64);
    let now = Utc::now();

    if now >= delay_until {
        // Delay period has already passed
        debug!(
            "Delay period expired (aired: {}, delay: {}min, expired: {})",
            air_date, delay_minutes, delay_until
        );
        DelayDecision::GrabNow
    } else {
        info!(
            "Delaying release: aired {}, delay {}min, grab after {}",
            air_date, delay_minutes, delay_until
        );
        DelayDecision::DelayUntil(delay_until)
    }
}

/// Protocol number to enum conversion (matching existing convention)
pub fn protocol_from_num(num: i32) -> Protocol {
    match num {
        1 => Protocol::Usenet,
        2 => Protocol::Torrent,
        _ => Protocol::Unknown,
    }
}

/// Protocol enum to number conversion
pub fn protocol_to_num(protocol: Protocol) -> i32 {
    match protocol {
        Protocol::Usenet => 1,
        Protocol::Torrent => 2,
        Protocol::Unknown => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_profile(
        id: i64,
        enable_usenet: bool,
        enable_torrent: bool,
        preferred_protocol: i32,
        usenet_delay: i32,
        torrent_delay: i32,
        tags: &[i32],
        order: i32,
    ) -> DelayProfileDbModel {
        DelayProfileDbModel {
            id,
            enable_usenet,
            enable_torrent,
            preferred_protocol,
            usenet_delay,
            torrent_delay,
            bypass_if_highest_quality: false,
            bypass_if_above_custom_format_score: 0,
            tags: serde_json::to_string(tags).unwrap(),
            order,
        }
    }

    // ========================================================================
    // Tag matching tests
    // ========================================================================

    #[test]
    fn test_find_matching_profile_no_profiles() {
        let profiles: Vec<DelayProfileDbModel> = vec![];
        assert!(find_matching_profile(&profiles, &[]).is_none());
    }

    #[test]
    fn test_find_matching_profile_default_only() {
        let profiles = vec![make_profile(1, true, true, 1, 720, 720, &[], 0)];
        let result = find_matching_profile(&profiles, &[1, 2]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 1);
    }

    #[test]
    fn test_find_matching_profile_tag_match() {
        let profiles = vec![
            make_profile(1, true, true, 1, 720, 720, &[], 0), // default
            make_profile(2, true, true, 2, 0, 0, &[5, 6], 1), // matches tag 5
        ];
        // Entity has tag 5 — should match profile 2
        let result = find_matching_profile(&profiles, &[5]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 2);
    }

    #[test]
    fn test_find_matching_profile_no_tag_match_uses_default() {
        let profiles = vec![
            make_profile(1, true, true, 1, 720, 720, &[], 0), // default
            make_profile(2, true, true, 2, 0, 0, &[5, 6], 1), // tags 5,6
        ];
        // Entity has tag 99 — no match, should fall back to default
        let result = find_matching_profile(&profiles, &[99]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 1);
    }

    #[test]
    fn test_find_matching_profile_first_tag_match_wins() {
        let profiles = vec![
            make_profile(1, true, true, 1, 720, 720, &[10], 0), // order 0, tag 10
            make_profile(2, true, true, 2, 0, 0, &[10], 1),     // order 1, tag 10
        ];
        // Both match tag 10, first in order wins
        let result = find_matching_profile(&profiles, &[10]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 1);
    }

    // ========================================================================
    // Delay calculation tests
    // ========================================================================

    #[test]
    fn test_check_delay_no_air_date() {
        let profile = make_profile(1, true, true, 1, 720, 720, &[], 0);
        assert_eq!(
            check_delay(&profile, Protocol::Torrent, None),
            DelayDecision::GrabNow
        );
    }

    #[test]
    fn test_check_delay_preferred_protocol_no_delay() {
        // Preferred = usenet (1), release is usenet — grab immediately
        let profile = make_profile(1, true, true, 1, 720, 720, &[], 0);
        let air_date = Utc::now(); // Just aired
        assert_eq!(
            check_delay(&profile, Protocol::Usenet, Some(air_date)),
            DelayDecision::GrabNow
        );
    }

    #[test]
    fn test_check_delay_non_preferred_protocol_delays() {
        // Preferred = usenet (1), release is torrent — should delay
        let profile = make_profile(1, true, true, 1, 720, 720, &[], 0);
        let air_date = Utc::now(); // Just aired, 720 min delay
        match check_delay(&profile, Protocol::Torrent, Some(air_date)) {
            DelayDecision::DelayUntil(until) => {
                // Should be approximately 720 minutes from now
                let expected = air_date + Duration::minutes(720);
                let diff = (until - expected).num_seconds().abs();
                assert!(diff < 2, "Delay until should be ~720min from air date");
            }
            DelayDecision::GrabNow => panic!("Expected delay, got GrabNow"),
        }
    }

    #[test]
    fn test_check_delay_expired_grabs_now() {
        // Preferred = usenet (1), release is torrent, but aired 2 days ago
        let profile = make_profile(1, true, true, 1, 720, 720, &[], 0);
        let air_date = Utc::now() - Duration::hours(48); // Aired 2 days ago
        assert_eq!(
            check_delay(&profile, Protocol::Torrent, Some(air_date)),
            DelayDecision::GrabNow
        );
    }

    #[test]
    fn test_check_delay_zero_delay_grabs_now() {
        // Torrent delay is 0 — no delay regardless of preference
        let profile = make_profile(1, true, true, 1, 720, 0, &[], 0);
        let air_date = Utc::now();
        assert_eq!(
            check_delay(&profile, Protocol::Torrent, Some(air_date)),
            DelayDecision::GrabNow
        );
    }

    #[test]
    fn test_check_delay_protocol_not_enabled() {
        // Torrent not enabled — grab immediately
        let profile = make_profile(1, true, false, 1, 720, 720, &[], 0);
        let air_date = Utc::now();
        assert_eq!(
            check_delay(&profile, Protocol::Torrent, Some(air_date)),
            DelayDecision::GrabNow
        );
    }

    #[test]
    fn test_check_delay_unknown_protocol() {
        let profile = make_profile(1, true, true, 1, 720, 720, &[], 0);
        let air_date = Utc::now();
        assert_eq!(
            check_delay(&profile, Protocol::Unknown, Some(air_date)),
            DelayDecision::GrabNow
        );
    }

    // ========================================================================
    // Protocol conversion tests
    // ========================================================================

    #[test]
    fn test_protocol_conversions() {
        assert_eq!(protocol_from_num(1), Protocol::Usenet);
        assert_eq!(protocol_from_num(2), Protocol::Torrent);
        assert_eq!(protocol_from_num(0), Protocol::Unknown);
        assert_eq!(protocol_from_num(99), Protocol::Unknown);

        assert_eq!(protocol_to_num(Protocol::Usenet), 1);
        assert_eq!(protocol_to_num(Protocol::Torrent), 2);
        assert_eq!(protocol_to_num(Protocol::Unknown), 0);
    }
}
