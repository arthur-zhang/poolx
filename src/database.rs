use std::fmt::Debug;

use crate::conn::Connection;

pub trait Database: 'static + Sized + Send + Debug {
    type Connection: Connection<Database=Self>;
}