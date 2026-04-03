//! CADMATIC REST API HTTP client.
//!
//! Wraps `reqwest` with:
//! - Authorization header injection
//! - Exponential-backoff retry on 5xx and connection errors
//! - JSON deserialisation to CADMATIC response types
//! - Structured error reporting

use crate::api::*;
use crate::config::CadmaticConfig;
use thiserror::Error;

/// Errors from the CADMATIC HTTP client.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("HTTP error {status} on {url}: {body}")]
    Http { status: u16, url: String, body: String },

    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("JSON deserialisation error for {url}: {source}")]
    Json { url: String, #[source] source: serde_json::Error },

    #[error("API returned empty response for {0}")]
    EmptyResponse(String),

    #[error("Authentication failed for {0}: check credentials")]
    AuthFailed(String),
}

/// Thin async HTTP client for the CADMATIC REST API.
pub struct CadmaticClient {
    http: reqwest::Client,
    config: CadmaticConfig,
}

impl CadmaticClient {
    /// Create a new client. Does not connect yet — call [`ping`] to test.
    pub fn new(config: CadmaticConfig) -> Result<Self, ClientError> {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(auth) = config.auth.header_value() {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                auth.parse().expect("valid auth header"),
            );
        }
        headers.insert(
            reqwest::header::ACCEPT,
            "application/json".parse().unwrap(),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            format!("pmef-adapter-cadmatic/0.9.0 (pmef-core/0.9.0)")
                .parse().unwrap(),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()?;

        Ok(Self { http, config })
    }

    /// Test connectivity — calls `GET /api/v1/projects`.
    pub async fn ping(&self) -> Result<(), ClientError> {
        let url = self.config.api_url("/projects");
        self.get_raw(&url).await.map(|_| ())
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    async fn get_raw(&self, url: &str) -> Result<serde_json::Value, ClientError> {
        let mut last_err = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = std::time::Duration::from_millis(200 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
                tracing::debug!("Retry {attempt} for {url}");
            }
            match self.http.get(url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status == 401 || status == 403 {
                        return Err(ClientError::AuthFailed(url.to_owned()));
                    }
                    if status.is_server_error() {
                        last_err = Some(ClientError::Http {
                            status: status.as_u16(),
                            url: url.to_owned(),
                            body: resp.text().await.unwrap_or_default(),
                        });
                        continue; // retry on 5xx
                    }
                    if !status.is_success() {
                        return Err(ClientError::Http {
                            status: status.as_u16(),
                            url: url.to_owned(),
                            body: resp.text().await.unwrap_or_default(),
                        });
                    }
                    let text = resp.text().await
                        .map_err(ClientError::Request)?;
                    return serde_json::from_str(&text)
                        .map_err(|e| ClientError::Json { url: url.to_owned(), source: e });
                }
                Err(e) if e.is_connect() || e.is_timeout() => {
                    last_err = Some(ClientError::Request(e));
                    continue; // retry on connection errors
                }
                Err(e) => return Err(ClientError::Request(e)),
            }
        }
        Err(last_err.unwrap_or_else(|| ClientError::EmptyResponse(url.to_owned())))
    }

    async fn get_list<T: serde::de::DeserializeOwned>(
        &self, url: &str,
    ) -> Result<Vec<T>, ClientError> {
        let val = self.get_raw(url).await?;
        // API may return either a top-level array or { "items": [...] }
        let arr = if val.is_array() {
            val
        } else if let Some(items) = val.get("items") {
            items.clone()
        } else if let Some(data) = val.get("data") {
            data.clone()
        } else {
            return Err(ClientError::EmptyResponse(url.to_owned()));
        };
        serde_json::from_value(arr)
            .map_err(|e| ClientError::Json { url: url.to_owned(), source: e })
    }

    // ── Public API methods ────────────────────────────────────────────────────

    /// `GET /api/v1/projects`
    pub async fn get_projects(&self) -> Result<Vec<CadmaticProject>, ClientError> {
        let url = self.config.api_url("/projects");
        self.get_list(&url).await
    }

    /// `GET /api/v1/projects/{projectId}/pipelines`
    pub async fn get_pipelines(&self, project_id: &str) -> Result<Vec<CadmaticLine>, ClientError> {
        let url = self.config.api_url(&format!("/projects/{project_id}/pipelines"));
        self.get_list(&url).await
    }

    /// `GET /api/v1/pipelines/{lineId}/components`
    ///
    /// Fetches all components in a piping line (paginated internally).
    pub async fn get_components(
        &self,
        line_id: &str,
    ) -> Result<Vec<CadmaticComponent>, ClientError> {
        let batch = self.config.component_batch_size;
        let mut all: Vec<CadmaticComponent> = Vec::new();
        let mut offset = 0usize;
        loop {
            let url = self.config.api_url(
                &format!("/pipelines/{line_id}/components?offset={offset}&limit={batch}")
            );
            let page: Vec<CadmaticComponent> = self.get_list(&url).await?;
            let page_len = page.len();
            all.extend(page);
            if page_len < batch { break; }
            offset += batch;
        }
        Ok(all)
    }

    /// `GET /api/v1/equipment`
    pub async fn get_equipment(&self) -> Result<Vec<CadmaticEquipment>, ClientError> {
        let url = self.config.api_url("/equipment");
        let mut equipment: Vec<CadmaticEquipment> = self.get_list(&url).await?;
        // Fetch nozzles for each equipment item
        for equip in &mut equipment {
            if let Ok(conn) = self.get_equipment_connections(&equip.object_guid).await {
                equip.nozzles = conn.nozzles;
            }
        }
        Ok(equipment)
    }

    /// `GET /api/v1/equipment/{id}/connections`
    pub async fn get_equipment_connections(
        &self,
        equipment_guid: &str,
    ) -> Result<CadmaticConnection, ClientError> {
        let url = self.config.api_url(&format!("/equipment/{equipment_guid}/connections"));
        let val = self.get_raw(&url).await?;
        serde_json::from_value(val)
            .map_err(|e| ClientError::Json { url: url.clone(), source: e })
    }

    /// `GET /api/v1/export/3ddx`  (full plant geometry export)
    ///
    /// Returns raw bytes of the 3DDX file. May be large (100s of MB for large plants).
    pub async fn export_3ddx(&self) -> Result<Vec<u8>, ClientError> {
        let url = self.config.api_url("/export/3ddx");
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(ClientError::Http {
                status: resp.status().as_u16(),
                url,
                body: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(resp.bytes().await?.to_vec())
    }

    /// `POST /api/v1/import/pmef`  (bulk PMEF import)
    pub async fn post_import_pmef(
        &self,
        objects: &[serde_json::Value],
    ) -> Result<serde_json::Value, ClientError> {
        let url = self.config.api_url("/import/pmef");
        let resp = self.http
            .post(&url)
            .json(objects)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ClientError::Http {
                status: resp.status().as_u16(),
                url,
                body: resp.text().await.unwrap_or_default(),
            });
        }
        let text = resp.text().await?;
        serde_json::from_str(&text)
            .map_err(|e| ClientError::Json { url: url.clone(), source: e })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock client for unit testing
// ─────────────────────────────────────────────────────────────────────────────

/// A mock CADMATIC client that returns canned responses.
/// Used in unit tests without a live CADMATIC server.
#[cfg(test)]
pub mod mock {
    use super::*;
    use crate::api::*;

    pub fn mock_pump() -> CadmaticEquipment {
        CadmaticEquipment {
            object_guid: "GUID-P-201A-0001".to_owned(),
            tag_number: "P-201A".to_owned(),
            equipment_type: "CentrifugalPump".to_owned(),
            description: Some("Cooling water pump".to_owned()),
            design_code: Some("API 610".to_owned()),
            train_id: Some("A".to_owned()),
            weight_kg: Some(1850.0),
            empty_weight_kg: Some(1650.0),
            operating_weight_kg: Some(1850.0),
            bbox_min: Some(CadmaticPoint3D { x: 10050., y: 5200., z: 700. }),
            bbox_max: Some(CadmaticPoint3D { x: 10450., y: 5700., z: 1600. }),
            area_code: Some("U-100".to_owned()),
            manufacturer: Some("Flowserve".to_owned()),
            model: Some("LCSA 200-315".to_owned()),
            nozzles: vec![
                CadmaticNozzle {
                    nozzle_id: "SUCTION".to_owned(),
                    nozzle_mark: Some("N1".to_owned()),
                    service: Some("Suction inlet".to_owned()),
                    nominal_diameter_mm: Some(200.0),
                    flange_rating: Some("ANSI-150".to_owned()),
                    facing_type: Some("RF".to_owned()),
                    position: CadmaticPoint3D { x: 10200., y: 5400., z: 850. },
                    direction: Some(CadmaticPoint3D { x: -1., y: 0., z: 0. }),
                    connected_line_id: None,
                },
                CadmaticNozzle {
                    nozzle_id: "DISCHARGE".to_owned(),
                    nozzle_mark: Some("N2".to_owned()),
                    service: Some("Discharge outlet".to_owned()),
                    nominal_diameter_mm: Some(150.0),
                    flange_rating: Some("ANSI-150".to_owned()),
                    facing_type: Some("RF".to_owned()),
                    position: CadmaticPoint3D { x: 10200., y: 5400., z: 1250. },
                    direction: Some(CadmaticPoint3D { x: 0., y: 0., z: 1. }),
                    connected_line_id: None,
                },
            ],
            custom_attributes: Default::default(),
        }
    }

    pub fn mock_line() -> CadmaticLine {
        CadmaticLine {
            line_id: "LINE-CW-201-GUID".to_owned(),
            line_number: "8\"-CW-201-A1A2".to_owned(),
            nominal_diameter: Some(200.0),
            pipe_class: Some("A1A2".to_owned()),
            fluid_code: Some("CW".to_owned()),
            fluid_description: Some("Cooling water".to_owned()),
            design_pressure_barg: Some(15.0),
            design_temperature_degc: Some(60.0),
            operating_pressure_barg: Some(5.0),
            operating_temperature_degc: Some(30.0),
            test_pressure_barg: Some(22.5),
            schedule: Some("SCH40".to_owned()),
            outside_diameter_mm: Some(219.1),
            wall_thickness_mm: Some(8.18),
            material: Some("A106B".to_owned()),
            insulation_type: Some("NONE".to_owned()),
            pid_reference: Some("P&ID-U100-001-Rev3".to_owned()),
            dexpi_ref: None,
            component_count: Some(5),
            modified_date: Some("2026-03-31T00:00:00Z".to_owned()),
        }
    }

    pub fn mock_pipe_component() -> CadmaticComponent {
        CadmaticComponent {
            object_guid: "GUID-PIPE-001".to_owned(),
            component_type: "StraightPipe".to_owned(),
            spec_key: Some("PIPW".to_owned()),
            item_number: Some("1".to_owned()),
            tag_number: None,
            material: Some("A106B".to_owned()),
            nominal_diameter_mm: Some(200.0),
            end_points: vec![
                CadmaticEndPoint {
                    index: 0, bore_mm: Some(200.0), end_type: Some("BW".to_owned()),
                    position: CadmaticPoint3D { x: 9000., y: 5400., z: 850. },
                    direction: Some(CadmaticPoint3D { x: -1., y: 0., z: 0. }),
                    connected_to_guid: None,
                },
                CadmaticEndPoint {
                    index: 1, bore_mm: Some(200.0), end_type: Some("BW".to_owned()),
                    position: CadmaticPoint3D { x: 11500., y: 5400., z: 850. },
                    direction: Some(CadmaticPoint3D { x: 1., y: 0., z: 0. }),
                    connected_to_guid: Some("GUID-ELB-001".to_owned()),
                },
            ],
            weight_kg: Some(85.4),
            catalogue_ref: Some("A1A2-PIPE-200".to_owned()),
            vendor: None,
            custom_attributes: Default::default(),
            angle_deg: None, bend_radius_mm: None,
            large_bore_mm: None, small_bore_mm: None,
            actuator_type: None, fail_position: None,
            weld_number: None, nde_method: None,
        }
    }

    pub fn mock_elbow_component() -> CadmaticComponent {
        CadmaticComponent {
            object_guid: "GUID-ELB-001".to_owned(),
            component_type: "Elbow90LR".to_owned(),
            spec_key: Some("ELBWLR90".to_owned()),
            item_number: Some("2".to_owned()),
            tag_number: None,
            material: Some("A234WPB".to_owned()),
            nominal_diameter_mm: Some(200.0),
            end_points: vec![
                CadmaticEndPoint {
                    index: 0, bore_mm: Some(200.0), end_type: Some("BW".to_owned()),
                    position: CadmaticPoint3D { x: 11500., y: 5400., z: 850. },
                    direction: Some(CadmaticPoint3D { x: -1., y: 0., z: 0. }),
                    connected_to_guid: Some("GUID-PIPE-001".to_owned()),
                },
                CadmaticEndPoint {
                    index: 1, bore_mm: Some(200.0), end_type: Some("BW".to_owned()),
                    position: CadmaticPoint3D { x: 11500., y: 5400., z: 1150. },
                    direction: Some(CadmaticPoint3D { x: 0., y: 0., z: -1. }),
                    connected_to_guid: None,
                },
            ],
            weight_kg: Some(18.6),
            catalogue_ref: Some("A1A2-ELB90LR-200".to_owned()),
            angle_deg: Some(90.0),
            bend_radius_mm: Some(304.8),
            vendor: None,
            custom_attributes: Default::default(),
            large_bore_mm: None, small_bore_mm: None,
            actuator_type: None, fail_position: None,
            weld_number: None, nde_method: None,
        }
    }
}
