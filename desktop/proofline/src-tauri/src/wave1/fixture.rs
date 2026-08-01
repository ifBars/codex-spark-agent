use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CANONICAL_MANIFEST_SHA: &str =
    "7829776e9aea00a0d182d00cddc3337f07659d728fbea9b31b30fdc05f36b3bf";

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
}

impl BundleFixture {
    pub(crate) fn bundled() -> Self {
        Self {
            manifest_bytes: include_bytes!("../../fixtures/wave1-manifest.json").to_vec(),
            evidence_bytes: include_bytes!("../../fixtures/ownership-map.lf.md").to_vec(),
        }
    }

    pub(crate) fn preflight(&self) -> Result<VerifiedFixture, String> {
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
        let canonical = serde_json::to_vec(&manifest)
            .map_err(|_| "bundled fixture manifest cannot be canonicalized".to_owned())?;
        let manifest_sha = hex_sha256(&canonical);
        if manifest_sha != CANONICAL_MANIFEST_SHA {
            return Err("bundled fixture manifest canonical identity is invalid".into());
        }
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
