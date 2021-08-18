use super::*;

impl<T: KeyhouseImpl + 'static> KeyhouseService<T> {
    async fn get_legacy_key(
        &self,
        spiffe_id: Option<&SpiffeID>,
        request: keyhouse::GetLegacyKeyRequest,
    ) -> Result<StdResult<keyhouse::GetLegacyKeyResponse, ErrorCode>> {
        let key = match self.load_key_from_alias(&request.alias).await? {
            Err(code) => return Ok(Err(code)),
            Ok(x) => x,
        };
        if key.purpose != KeyPurpose::EncodeDecode {
            return Ok(Err(ErrorCode::Forbidden));
        }
        match KeyhouseService::<T>::authorize_acls(
            spiffe_id,
            key.acls.get(&AccessControlDomain::Encode),
        ) {
            Ok(_) => (),
            Err(code) => return Ok(Err(code)),
        }

        let data_key = key.decode_legacy_key::<T>()?;

        Ok(Ok(keyhouse::GetLegacyKeyResponse {
            error_code: ErrorCode::Ok as i32,
            data_key,
        }))
    }

    pub(crate) async fn get_legacy_key_wrap(
        &self,
        raw_request: Request<keyhouse::GetLegacyKeyRequest>,
        spiffe_id: Option<SpiffeID>,
        ip: String,
    ) -> StdResult<KeyhouseResponse<keyhouse::GetLegacyKeyResponse>, Status> {
        let spiffe_id_value = spiffe_id.as_ref().map(|x| x.to_string());
        let (token_spiffe_id, token_value) =
            KeyhouseService::<T>::extract_alt_token(&raw_request.get_ref().token);
        let total_spiffe_id = spiffe_id.as_ref().or_else(|| token_spiffe_id.as_ref());
        let (auth_service, auth_user) = Self::get_auth_user_service(total_spiffe_id);

        let request = raw_request.into_inner();
        let key_alias = Some(request.alias.clone());
        let result = self.get_legacy_key(total_spiffe_id, request).await;

        let error_code = match &result {
            Ok(Ok(_)) => ErrorCode::Ok,
            Ok(Err(code)) => *code,
            Err(_) => ErrorCode::Unknown,
        };

        LogEvent::DataLogEvent(DataLogEvent {
            occurred_at: crate::util::epoch(),
            request_type: DataRequestType::GetLegacyKey,
            spiffe_id: spiffe_id_value,
            token: token_value,
            auth_service,
            auth_user,
            ip,
            key_id: None,
            key_alias: key_alias.clone(),
            data_key_hash: None,
            status: error_code,
            internal_failure: result.is_err(),
            message: result.as_ref().err().map(|x| format!("{:?}", x)),
        })
        .fire::<T>();

        let response = result
            .unwrap_or(Err(ErrorCode::Unknown))
            .unwrap_or_else(|e| keyhouse::GetLegacyKeyResponse {
                error_code: e as i32,
                data_key: vec![],
            });
        Ok(KeyhouseResponse {
            response,
            spiffe_id: total_spiffe_id.cloned(),
            error_code,
            target_alias: key_alias,
        })
    }
}