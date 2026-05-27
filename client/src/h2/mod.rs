pub(crate) mod body;
pub(crate) mod proto;

use h2::client::SendRequest;
use xitca_http::bytes::Bytes;

pub type Connection = SendRequest<Bytes>;
