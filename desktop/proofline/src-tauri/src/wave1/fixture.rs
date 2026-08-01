use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CANONICAL_MANIFEST_SHA: &str =
    "29415e0ce8f7659093e01032ce52365197c59d0010bad7aa4361048fdb86abe5";
const RENDERER_FIXTURE_SOURCE_SHA: &str =
    "0a43f614e844798079b5244c16dffff694afd89c9f6328501117db5da1573a1d";

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Manifest {
    schema: String,
    fixture_id: String,
    revision: String,
    runtime_mode: String,
    evidence_files: Vec<EvidenceFile>,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct EvidenceFile {
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Scenario {
    task_id: String,
    outcome: String,
}

#[derive(Clone)]
pub(crate) struct BundleFixture {
    manifest_bytes: Vec<u8>,
    evidence_bytes: Vec<u8>,
    renderer_source_bytes: Vec<u8>,
}

impl BundleFixture {
    pub(crate) fn bundled() -> Self {
        Self {
            manifest_bytes: include_bytes!("../../fixtures/wave1-manifest.json").to_vec(),
            evidence_bytes: include_bytes!("../../../fixtures/ownership-map.md").to_vec(),
            renderer_source_bytes: include_bytes!("../../../src/wave1-replay.js").to_vec(),
        }
    }

    pub(crate) fn preflight(&self) -> Result<VerifiedFixture, String> {
        let manifest_sha = hex_sha256(&self.manifest_bytes);
        if manifest_sha != CANONICAL_MANIFEST_SHA {
            return Err("bundled fixture manifest byte identity is invalid".into());
        }
        let manifest: Manifest = serde_json::from_slice(&self.manifest_bytes)
            .map_err(|_| "bundled fixture manifest is malformed".to_owned())?;
        if manifest.schema != "spark.proofline.fixture.v1"
            || manifest.fixture_id != "proofline-wave1-local"
            || manifest.revision.is_empty()
            || manifest.runtime_mode != "replayed"
        {
            return Err("bundled fixture manifest identity is invalid".into());
        }
        if manifest
            .scenarios
            .iter()
            .map(|scenario| scenario.task_id.as_str())
            .collect::<Vec<_>>()
            != [
                "proofline-1",
                "proofline-2",
                "proofline-3",
                "proofline-4",
                "proofline-5",
            ]
            || manifest
                .scenarios
                .iter()
                .any(|scenario| scenario.outcome.is_empty())
        {
            return Err("bundled fixture manifest scenarios are invalid".into());
        }
        let expected = manifest
            .evidence_files
            .iter()
            .find(|file| file.path == "fixtures/ownership-map.md")
            .ok_or_else(|| "bundled fixture manifest has no ownership evidence".to_owned())?;
        let actual = hex_sha256(&self.evidence_bytes);
        if actual != expected.sha256 {
            return Err("bundled fixture evidence does not match its manifest".into());
        }
        self.verify_renderer_fixture_source(&manifest, expected)?;
        Ok(VerifiedFixture {
            id: manifest.fixture_id,
            revision: manifest.revision,
            sha256: manifest_sha,
        })
    }

    #[cfg(test)]
    pub(crate) fn tampered() -> Self {
        let mut fixture = Self::bundled();
        fixture.evidence_bytes.push(b'!');
        fixture
    }

    #[cfg(test)]
    pub(crate) fn renderer_source_tampered() -> Self {
        let mut fixture = Self::bundled();
        fixture.renderer_source_bytes.push(b'!');
        fixture
    }

    #[cfg(test)]
    pub(crate) fn manifest_bytes_changed() -> Self {
        let mut fixture = Self::bundled();
        fixture.manifest_bytes.push(b' ');
        fixture
    }

    fn verify_renderer_fixture_source(
        &self,
        manifest: &Manifest,
        evidence: &EvidenceFile,
    ) -> Result<(), String> {
        if hex_sha256(&self.renderer_source_bytes) != RENDERER_FIXTURE_SOURCE_SHA {
            return Err(
                "displayed renderer fixture source bytes do not match the native bundle".into(),
            );
        }
        let source = std::str::from_utf8(&self.renderer_source_bytes)
            .map_err(|_| "displayed renderer fixture source is not UTF-8".to_owned())?;
        let expected_id = format!(
            "export const WAVE1_FIXTURE_ID = \"{}\"",
            manifest.fixture_id
        );
        let expected_revision = format!(
            "export const WAVE1_FIXTURE_REVISION = \"{}\"",
            manifest.revision
        );
        let expected_manifest_sha = format!(
            "export const WAVE1_FIXTURE_SHA256 = \"{}\"",
            CANONICAL_MANIFEST_SHA
        );
        let expected_evidence = format!(
            "path: \"{}\", sha256: \"{}\"",
            evidence.path, evidence.sha256
        );
        if [
            expected_id,
            expected_revision,
            expected_manifest_sha,
            expected_evidence,
        ]
        .iter()
        .any(|needle| !source.contains(needle))
        {
            return Err(
                "displayed renderer fixture identity does not match the native bundle".into(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct VerifiedFixture {
    pub(crate) id: String,
    pub(crate) revision: String,
    pub(crate) sha256: String,
}

pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
