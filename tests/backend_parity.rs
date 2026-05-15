#![cfg(all(not(target_arch = "wasm32"), feature = "parity-tests"))]

#[test]
fn backend_parity_harness_runs() {
    let report = aion_charts::core::renderer::backend_parity_tests::run_backend_parity_harness()
        .expect("backend parity harness should run");

    if let Some(failure) = report.results.iter().find(|result| !result.passed) {
        panic!(
            "fixture {} failed parity validation: {}",
            failure.name, failure.note
        );
    }

    assert!(
        report.report_path.exists(),
        "expected parity report at {:?}",
        report.report_path
    );
}
