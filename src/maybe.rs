use std::ops::{Deref, DerefMut};
use crate::conn::Connection;
use crate::PoolConnection;

pub enum MaybePoolConnection<'c, C: Connection> {
    #[allow(dead_code)]
    Connection(&'c mut C),
    PoolConnection(PoolConnection<C>),
}

impl<'c, C: Connection> Deref for MaybePoolConnection<'c, C> {
    type Target = C;

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            MaybePoolConnection::Connection(v) => v,
            MaybePoolConnection::PoolConnection(v) => v,
        }
    }
}

impl<'c, C: Connection> DerefMut for MaybePoolConnection<'c, C> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            MaybePoolConnection::Connection(v) => v,
            MaybePoolConnection::PoolConnection(v) => v,
        }
    }
}

impl<'c, C: Connection> From<PoolConnection<C>> for MaybePoolConnection<'c, C> {
    fn from(v: PoolConnection<C>) -> Self {
        MaybePoolConnection::PoolConnection(v)
    }
}

impl<'c, C: Connection> From<&'c mut C> for MaybePoolConnection<'c, C> {
    fn from(v: &'c mut C) -> Self {
        MaybePoolConnection::Connection(v)
    }
}
