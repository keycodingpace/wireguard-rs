use lazy_static::lazy_static;
use spin::Mutex;
use std::time::{Duration, Instant};

use generic_array::typenum::U32;
use generic_array::GenericArray;

use x25519_dalek::PublicKey;
use x25519_dalek::SharedSecret;
use x25519_dalek::StaticSecret;

use super::device::Device;
use super::macs;
use super::timestamp;
use super::types::*;

lazy_static! {
    pub static ref TIME_BETWEEN_INITIATIONS: Duration = Duration::from_millis(20);
}

/* Represents the recomputation and state of a peer.
 *
 * This type is only for internal use and not exposed.
 */
pub struct Peer<T> {
    // external identifier
    pub(crate) identifier: T,

    // mutable state
    state: Mutex<State>,
    timestamp: Mutex<Option<timestamp::TAI64N>>,
    last_initiation_consumption: Mutex<Option<Instant>>,

    // state related to DoS mitigation fields
    pub(crate) macs: Mutex<macs::Generator>,

    // constant state
    pub(crate) pk: PublicKey,    // public key of peer
    pub(crate) ss: SharedSecret, // precomputed DH(static, static)
    pub(crate) psk: Psk,         // psk of peer
}

pub enum State {
    Reset,
    InitiationSent {
        sender: u32, // assigned sender id
        eph_sk: StaticSecret,
        hs: GenericArray<u8, U32>,
        ck: GenericArray<u8, U32>,
    },
}

impl Clone for State {
    fn clone(&self) -> State {
        match self {
            State::Reset => State::Reset,
            State::InitiationSent {
                sender,
                eph_sk,
                hs,
                ck,
            } => State::InitiationSent {
                sender: *sender,
                eph_sk: StaticSecret::from(eph_sk.to_bytes()),
                hs: *hs,
                ck: *ck,
            },
        }
    }
}

impl<T> Peer<T>
where
    T: Copy,
{
    pub fn new(
        identifier: T,    // external identifier
        pk: PublicKey,    // public key of peer
        ss: SharedSecret, // precomputed DH(static, static)
    ) -> Self {
        Self {
            macs: Mutex::new(macs::Generator::new(pk)),
            identifier: identifier,
            state: Mutex::new(State::Reset),
            timestamp: Mutex::new(None),
            last_initiation_consumption: Mutex::new(None),
            pk: pk,
            ss: ss,
            psk: [0u8; 32],
        }
    }

    /// Return the state of the peer
    ///
    /// # Arguments
    pub fn get_state(&self) -> State {
        self.state.lock().clone()
    }

    /// Set the state of the peer unconditionally
    ///
    /// # Arguments
    ///
    pub fn set_state(&self, state_new: State) {
        *self.state.lock() = state_new;
    }

    /// Set the mutable state of the peer conditioned on the timestamp being newer
    ///
    /// # Arguments
    ///
    /// * st_new - The updated state of the peer
    /// * ts_new - The associated timestamp
    pub fn check_replay_flood(
        &self,
        device: &Device<T>,
        timestamp_new: &timestamp::TAI64N,
    ) -> Result<(), HandshakeError> {
        let mut state = self.state.lock();
        let mut timestamp = self.timestamp.lock();
        let mut last_initiation_consumption = self.last_initiation_consumption.lock();

        // check replay attack
        match *timestamp {
            Some(timestamp_old) => {
                if !timestamp::compare(&timestamp_old, &timestamp_new) {
                    return Err(HandshakeError::OldTimestamp);
                }
            }
            _ => (),
        };

        // check flood attack
        match *last_initiation_consumption {
            Some(last) => {
                if last.elapsed() < *TIME_BETWEEN_INITIATIONS {
                    return Err(HandshakeError::InitiationFlood);
                }
            }
            _ => (),
        }

        // reset state
        match *state {
            State::InitiationSent { sender, .. } => device.release(sender),
            _ => (),
        }

        // update replay & flood protection
        *state = State::Reset;
        *timestamp = Some(*timestamp_new);
        *last_initiation_consumption = Some(Instant::now());
        Ok(())
    }
}
