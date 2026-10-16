use once_cell::sync::Lazy;
use ts_bookkeeping::messages::s2c::InMessage;
use tsproto_packets::packets::{Direction, Flags, OutPacket, PacketType};

static TRACING: Lazy<()> = Lazy::new(|| tracing_subscriber::fmt().with_test_writer().init());

pub(crate) fn create_logger() { Lazy::force(&TRACING); }

fn parse_msg(msg: &str) -> InMessage {
	let header = OutPacket::new_with_dir(Direction::S2C, Flags::empty(), PacketType::Command);

	InMessage::new(&header.header(), msg.as_bytes()).unwrap()
}

fn test_iconid(input: &str, expected: u32) {
	create_logger();

	let msg = parse_msg(&format!(
		r#"initserver virtualserver_name=TeamSpeak\s]I[\sServer virtualserver_welcomemessage=Welcome\sto\sTeamSpeak,\scheck\s[URL]www.teamspeak.com[\/URL]\sfor\slatest\sinformation virtualserver_platform=Linux virtualserver_version=3.11.0\s[Build:\s1578903157] virtualserver_maxclients=32 virtualserver_created=1571572631 virtualserver_codec_encryption_mode=2 virtualserver_hostmessage virtualserver_hostmessage_mode=0 virtualserver_default_server_group=8 virtualserver_default_channel_group=8 virtualserver_hostbanner_url virtualserver_hostbanner_gfx_url virtualserver_hostbanner_gfx_interval=0 virtualserver_priority_speaker_dimm_modificator=-18.0000 virtualserver_id=1 virtualserver_hostbutton_tooltip virtualserver_hostbutton_url virtualserver_hostbutton_gfx_url virtualserver_name_phonetic virtualserver_ip=0.0.0.0,\s:: virtualserver_ask_for_privilegekey=0 virtualserver_hostbanner_mode=0 virtualserver_channel_temp_delete_delay_default=0 virtualserver_nickname client_nickname=TeamSpeakUser client_version=3.?.?\s[Build:\s5680278000] client_platform=Windows client_input_muted=0 client_output_muted=0 client_outputonly_muted=0 client_input_hardware=1 client_output_hardware=1 client_default_channel client_default_channel_password client_server_password client_meta_data client_version_sign=DX5NIYLvfJEUjuIbCidnoeozxIDRRkpq3I9vVMBmE9L2qnekOoBzSenkzsg2lC9CMv8K5hkEzhr2TYUYSwUXCg== client_security_hash client_key_offset=354 client_away=0 client_away_message client_nickname_phonetic client_default_token client_badges client_myteamspeak_id client_integrations client_active_integrations_info client_myteamspeak_avatar client_signed_badges acn=TeamSpeakUser aclid=2 pv=7 client_talk_power=75 client_needed_serverquery_view_power=75 virtualserver_icon_id={}"#,
		input
	));
	if let InMessage::InitServer(list) = msg {
		let cmd = list.iter().next().unwrap();
		assert_eq!(cmd.icon, ts_bookkeeping::IconId(expected));
	} else {
		panic!("Failed to parse as initserver");
	}
}

#[test]
fn normal_iconid() { test_iconid("96136942", 96136942); }

#[test]
fn negative_iconid() { test_iconid("-96136942", 4198830354); }

#[test]
fn big_iconid() { test_iconid("18446744073225738240", 3811153920); }

#[tokio::test]
async fn disconnect_before_polling_does_not_start_a_handshake() {
	use crate::{Connection, DisconnectOptions, Identity};
	use futures::StreamExt;
	use std::time::Duration;
	let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
	let mut connection = Connection::build(server.local_addr().unwrap())
		.identity(Identity::create())
		.connect()
		.unwrap();
	connection.disconnect(DisconnectOptions::new()).unwrap();
	assert!(
		tokio::time::timeout(Duration::from_millis(100), connection.events().next())
			.await
			.unwrap()
			.is_none()
	);
	assert!(
		tokio::time::timeout(Duration::from_millis(50), server.recv_from(&mut [0; 1500]))
			.await
			.is_err()
	);
}

#[tokio::test]
async fn a_failed_connection_future_is_not_polled_after_completion() {
	use crate::{Connection, ConnectionState, Error, Identity, TsError};
	use futures::{FutureExt, StreamExt};
	let mut connection =
		Connection::build("127.0.0.1:1").identity(Identity::create()).connect().unwrap();
	connection.state = ConnectionState::Connecting(
		async { Err(Error::ConnectTs(TsError::ConnectFailedBanned)) }.boxed(),
		false,
	);
	assert!(matches!(connection.events().next().await, Some(Err(Error::ConnectTs(_)))));
	assert!(connection.events().next().await.is_none());
	assert!(connection.events().next().await.is_none());
}

#[tokio::test]
async fn an_identity_level_failure_closes_the_completed_connection_future() {
	use crate::{Connection, ConnectionState, Error, Identity};
	use futures::{FutureExt, StreamExt};
	let mut connection =
		Connection::build("127.0.0.1:1").identity(Identity::create()).connect().unwrap();
	connection.state =
		ConnectionState::Connecting(async { Err(Error::IdentityLevel(21)) }.boxed(), false);
	assert!(matches!(connection.events().next().await, Some(Err(Error::IdentityLevel(21)))));
	assert!(connection.events().next().await.is_none());
}

#[tokio::test]
async fn disconnect_preserves_a_completed_identity_upgrade_without_starting_a_login() {
	use crate::{
		Connection, ConnectionState, DisconnectOptions, Identity, IdentityIncreaseLevelState,
	};
	use futures::StreamExt;
	let seed = Identity::create();
	let mut identity = Identity::new(seed.key().clone(), 0);
	while identity.level() != 0 {
		identity.set_counter(identity.counter() + 1);
		identity.set_max_counter(identity.counter());
	}
	let mut upgraded = identity.clone();
	upgraded.upgrade_level(8);
	let (send, recv) = tokio::sync::oneshot::channel();
	assert!(send.send(upgraded.clone()).is_ok());
	let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
	let mut connection =
		Connection::build(server.local_addr().unwrap()).identity(identity).connect().unwrap();
	connection.state = ConnectionState::IdentityLevelIncreasing {
		recv,
		state: std::sync::Arc::new(std::sync::Mutex::new(IdentityIncreaseLevelState::Computing)),
	};
	connection.disconnect(DisconnectOptions::new()).unwrap();
	let preserved = connection.get_options().get_identity().unwrap();
	assert_eq!(preserved.counter(), upgraded.counter());
	assert_eq!(preserved.max_counter(), upgraded.max_counter());
	assert!(connection.events().next().await.is_none());
	assert!(
		tokio::time::timeout(
			std::time::Duration::from_millis(50),
			server.recv_from(&mut [0; 1500])
		)
		.await
		.is_err()
	);
}

#[tokio::test]
async fn disconnect_while_resolving_ends_the_connection() {
	use crate::{Connection, ConnectionState, DisconnectOptions, Identity};
	use futures::{FutureExt, StreamExt};
	use std::time::Duration;
	let mut connection =
		Connection::build("127.0.0.1:1").identity(Identity::create()).connect().unwrap();
	let options = connection.options.clone();
	connection.state = ConnectionState::Connecting(
		async move {
			Connection::connect_resolved(&options, futures::stream::pending()).await
		}
		.boxed(),
		false,
	);
	assert!(connection.events().next().now_or_never().is_none());
	connection.disconnect(DisconnectOptions::new()).unwrap();
	assert!(
		tokio::time::timeout(Duration::from_millis(100), connection.events().next())
			.await
			.expect("cancelled resolution must not wait for an address")
			.is_none()
	);
}

#[tokio::test]
async fn disconnect_during_handshake_does_not_poll_more_resolved_addresses() {
	use crate::{Connection, ConnectionState, DisconnectOptions, Identity};
	use futures::{FutureExt, StreamExt};
	use std::sync::atomic::{AtomicUsize, Ordering};
	use std::sync::Arc;
	use std::task::Poll;
	use std::time::Duration;
	let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
	let address = server.local_addr().unwrap();
	let mut connection = Connection::build(address).identity(Identity::create()).connect().unwrap();
	let options = connection.options.clone();
	let further_polls = Arc::new(AtomicUsize::new(0));
	let observed_polls = further_polls.clone();
	let resolved = futures::stream::once(futures::future::ready(Ok(address))).chain(
		futures::stream::poll_fn(move |_| {
			further_polls.fetch_add(1, Ordering::SeqCst);
			Poll::Pending
		}),
	);
	connection.state = ConnectionState::Connecting(
		async move { Connection::connect_resolved(&options, resolved).await }.boxed(),
		false,
	);
	let mut packet = [0; 1500];
	tokio::select! {
		result = server.recv_from(&mut packet) => {
			let (length, _) = result.unwrap();
			assert!(length >= 8 && &packet[..8] == b"TS3INIT1");
		},
		result = async { connection.events().next().await } => panic!("handshake ended before cancellation: {:?}", result),
		_ = tokio::time::sleep(Duration::from_secs(1)) => panic!("handshake did not start"),
	}
	connection.disconnect(DisconnectOptions::new()).unwrap();
	assert!(
		tokio::time::timeout(Duration::from_millis(100), connection.events().next())
			.await
			.expect("cancelled handshake must not continue resolving")
			.is_none()
	);
	assert_eq!(observed_polls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn disconnect_during_reconnect_delay_ends_without_a_handshake() {
	use crate::{Connection, ConnectionState, DisconnectOptions, Identity};
	use futures::{FutureExt, StreamExt};
	use std::time::Duration;
	let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
	let mut connection = Connection::build(server.local_addr().unwrap())
		.identity(Identity::create())
		.connect()
		.unwrap();
	connection.state = ConnectionState::Connecting(
		Connection::connect(connection.options.clone(), true).boxed(),
		true,
	);
	assert!(connection.events().next().now_or_never().is_none());
	connection.disconnect(DisconnectOptions::new()).unwrap();
	assert!(
		tokio::time::timeout(Duration::from_millis(100), connection.events().next())
			.await
			.expect("cancellation must not wait for the reconnect delay")
			.is_none()
	);
	assert!(
		tokio::time::timeout(Duration::from_millis(50), server.recv_from(&mut [0; 1500]))
			.await
			.is_err()
	);
}

#[tokio::test]
async fn reusing_options_from_a_cancelled_connection_starts_a_new_handshake() {
	use crate::{Connection, DisconnectOptions, Identity};
	use futures::StreamExt;
	use std::time::Duration;
	let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
	let mut cancelled = Connection::build(server.local_addr().unwrap())
		.identity(Identity::create())
		.connect()
		.unwrap();
	cancelled.disconnect(DisconnectOptions::new()).unwrap();
	let mut connection = cancelled.get_options().clone().connect().unwrap();
	let mut packet = [0; 1500];
	tokio::select! {
		result = server.recv_from(&mut packet) => {
			let (length, _) = result.unwrap();
			assert!(length >= 8 && &packet[..8] == b"TS3INIT1");
		},
		result = async { connection.events().next().await } => panic!("reused options did not start a connection: {:?}", result),
		_ = tokio::time::sleep(Duration::from_secs(1)) => panic!("reused options did not start a handshake"),
	}
	connection.disconnect(DisconnectOptions::new()).unwrap();
	assert!(
		tokio::time::timeout(Duration::from_millis(100), connection.events().next())
			.await
			.unwrap()
			.is_none()
	);
}

#[tokio::test]
async fn a_dropped_identity_upgrade_sender_closes_the_event_stream() {
	use crate::{Connection, ConnectionState, Error, Identity, IdentityIncreaseLevelState};
	use futures::StreamExt;
	let mut connection =
		Connection::build("127.0.0.1:1").identity(Identity::create()).connect().unwrap();
	let (send, recv) = tokio::sync::oneshot::channel();
	drop(send);
	connection.state = ConnectionState::IdentityLevelIncreasing {
		recv,
		state: std::sync::Arc::new(std::sync::Mutex::new(IdentityIncreaseLevelState::Computing)),
	};
	assert!(matches!(
		connection.events().next().await,
		Some(Err(Error::IdentityLevelIncreaseFailedThread))
	));
	assert!(connection.events().next().await.is_none());
	assert!(connection.events().next().await.is_none());
}
