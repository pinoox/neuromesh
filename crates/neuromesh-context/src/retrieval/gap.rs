use crate::retrieval::task_profile::TaskProfileKind;
use neuromesh_core::PacketGap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapSeverity {
    Critical,
    NonCritical,
}

#[derive(Debug, Clone)]
pub struct ClassifiedGap {
    pub label: String,
    pub severity: GapSeverity,
}

/// Classify packet gaps as critical (triggers L2/L3) vs non-critical (log only).
pub fn classify_gaps(gaps: &[PacketGap], profile: TaskProfileKind) -> Vec<ClassifiedGap> {
    gaps.iter()
        .map(|g| {
            let path_l = g.path.to_lowercase();
            let reason_l = g.reason.to_lowercase();
            let critical = is_critical_gap(&path_l, &reason_l, profile);
            ClassifiedGap {
                label: format!("{}: {}", g.path, g.reason),
                severity: if critical {
                    GapSeverity::Critical
                } else {
                    GapSeverity::NonCritical
                },
            }
        })
        .collect()
}

fn is_critical_gap(path: &str, reason: &str, profile: TaskProfileKind) -> bool {
    let role_keywords = profile.role_keywords();
    if role_keywords
        .iter()
        .any(|kw| path.contains(kw) || reason.contains(kw))
    {
        return true;
    }
    // Auth/middleware/routing gaps are always critical for their profiles.
    match profile {
        TaskProfileKind::Middleware => {
            path.contains("middleware") || reason.contains("middleware") || reason.contains("auth")
        }
        TaskProfileKind::Routing => {
            path.contains("route") || reason.contains("route") || reason.contains("router")
        }
        TaskProfileKind::SessionAuth => {
            path.contains("auth")
                || path.contains("session")
                || reason.contains("auth")
                || reason.contains("session")
        }
        TaskProfileKind::Impact | TaskProfileKind::DependencyTrace => {
            reason.contains("caller") || reason.contains("callee") || reason.contains("depend")
        }
        _ => reason.contains("required") || reason.contains("missing seed"),
    }
}
