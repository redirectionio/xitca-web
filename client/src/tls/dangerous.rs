#[cfg(feature = "rustls-ring-crypto")]
pub(crate) mod rustls {
    use std::sync::Arc;
    use xitca_tls::rustls_poll::{
        self, DigitallySignedStruct,
        client::danger::HandshakeSignatureValid,
        crypto::{verify_tls12_signature, verify_tls13_signature},
        pki_types::{CertificateDer, ServerName, UnixTime},
    };

    #[derive(Debug)]
    pub(crate) struct SkipServerVerification;

    impl SkipServerVerification {
        pub(crate) fn new() -> Arc<Self> {
            Arc::new(Self)
        }
    }

    impl rustls_poll::client::danger::ServerCertVerifier for SkipServerVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp: &[u8],
            _now: UnixTime,
        ) -> Result<rustls_poll::client::danger::ServerCertVerified, rustls_poll::Error> {
            Ok(rustls_poll::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls_poll::Error> {
            verify_tls12_signature(
                message,
                cert,
                dss,
                &rustls_poll::crypto::ring::default_provider().signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls_poll::Error> {
            verify_tls13_signature(
                message,
                cert,
                dss,
                &rustls_poll::crypto::ring::default_provider().signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<rustls_poll::SignatureScheme> {
            rustls_poll::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }
}
