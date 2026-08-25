/// Resolves the caller's backend request against what this build actually
/// compiled in. `has_*` are passed rather than read from `cfg!` so the rejection
/// paths are testable on the host, where neither feature is meaningful.
pub fn backends_for(
    requested: Option<&str>,
    has_webgpu: bool,
    has_webgl2: bool,
) -> Result<wgpu::Backends, String> {
    let webgpu = match has_webgpu {
        true => wgpu::Backends::BROWSER_WEBGPU,
        false => wgpu::Backends::empty(),
    };
    let webgl2 = match has_webgl2 {
        true => wgpu::Backends::GL,
        false => wgpu::Backends::empty(),
    };
    let selected = match requested {
        None | Some("auto") => webgpu | webgl2,
        Some("webgpu") => webgpu,
        Some("webgl2") => webgl2,
        Some(other) => {
            return Err(format!(
                "unknown backend {other:?}: expected \"auto\", \"webgpu\" or \"webgl2\""
            ))
        }
    };
    if selected.is_empty() {
        return Err(match requested {
            Some(name) => format!("this build does not include the {name} backend"),
            None => "this build includes no browser backend at all".to_string(),
        });
    }
    Ok(selected)
}

pub fn backend_name(backend: wgpu::Backend) -> &'static str {
    match backend {
        wgpu::Backend::BrowserWebGpu => "webgpu",
        wgpu::Backend::Gl => "webgl2",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_offers_both_backends_when_both_are_compiled_in() {
        let both = backends_for(None, true, true).expect("auto");
        assert!(both.contains(wgpu::Backends::BROWSER_WEBGPU));
        assert!(both.contains(wgpu::Backends::GL));
        assert_eq!(backends_for(Some("auto"), true, true), Ok(both));
    }

    #[test]
    fn an_explicit_request_narrows_to_that_backend_alone() {
        assert_eq!(
            backends_for(Some("webgpu"), true, true),
            Ok(wgpu::Backends::BROWSER_WEBGPU)
        );
        assert_eq!(
            backends_for(Some("webgl2"), true, true),
            Ok(wgpu::Backends::GL)
        );
    }

    /// A typo used to fall through to `auto`, which hid the mistake behind a
    /// backend the caller never asked for.
    #[test]
    fn a_misspelled_backend_is_rejected_rather_than_falling_back_to_auto() {
        let err = backends_for(Some("webgl"), true, true).expect_err("should reject");
        assert!(err.contains("webgl"), "{err}");
    }

    #[test]
    fn asking_for_a_backend_this_build_omits_reports_the_build_not_no_adapter() {
        let err = backends_for(Some("webgpu"), false, true).expect_err("should reject");
        assert!(err.contains("webgpu"), "{err}");
        assert!(backends_for(Some("webgl2"), false, true).is_ok());
    }

    #[test]
    fn auto_on_a_build_with_no_backends_fails_instead_of_returning_empty() {
        assert!(backends_for(None, false, false).is_err());
    }

    #[test]
    fn the_reported_backend_name_matches_the_string_the_caller_passes_in() {
        assert_eq!(backend_name(wgpu::Backend::BrowserWebGpu), "webgpu");
        assert_eq!(backend_name(wgpu::Backend::Gl), "webgl2");
        assert_eq!(backend_name(wgpu::Backend::Dx12), "unknown");
    }
}
