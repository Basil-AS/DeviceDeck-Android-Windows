//! Tests of the core crate against the shipping `protocol.rs`. Only the desktop has both.

/// The legacy decoders and builders next to the shared types: the reference side of the differential tests.
mod old_all {
    pub use crate::protocol::*;
    pub use crate::types::*;
}

mod wire {
    use buds_core::wire::*;

    fn msg(cmd: u16, seq: u8, n: usize) -> Message { Message { cmd, seq, payload: (0..n).map(|i| i as u8).collect() } }

    #[test]
    fn captured_frames() {
        // PROTOCOL.md section 2 examples
        assert_eq!(encode(&Message { cmd: 0x0100, seq: 1, payload: vec![] }, BUDGET), [vec![0xAA, 7, 0, 0, 0, 1, 1, 0, 0]]);
        assert_eq!(encode(&Message { cmd: 0x0404, seq: 6, payload: vec![1, 1, 1] }, BUDGET),
            [vec![0xAA, 0x0A, 0, 0, 4, 4, 6, 3, 0, 1, 1, 1]]);
        // Len above 127 takes two bytes: a 124-byte payload goes out as `AA 83 01`.
        assert_eq!(&encode(&msg(0x0418, 3, 124), BUDGET)[0][..3], [0xAA, 0x83, 0x01]);
    }

    #[test]
    fn same_bytes_as_the_shipping_encoder() {
        for n in [0, 1, 3, 100, 120, 121, 122, 250, 600] {
            let m = msg(0x0403, 9, n);
            assert_eq!(encode(&m, BUDGET), [crate::types::build_packet(m.cmd, m.seq, &m.payload)], "payload {n}");
        }
    }

    #[test]
    fn slices_join_again() {
        for (n, budget) in [(1500, BUDGET), (600, 100), (5000, 129), (700, 660)] {
            let m = msg(0x8115, 4, n);
            let frames = encode(&m, budget);
            assert!(frames.len() > 1 || n < 660);
            assert!(frames.iter().all(|f| f.len() <= budget), "frame over budget {budget}");
            let mut r = Reassembler::default();
            let got: Vec<Message> = frames.iter().flat_map(|f| r.push(f)).collect();
            assert_eq!(got, [m], "payload {n} budget {budget}");
        }
    }

    #[test]
    fn stream_in_pieces_and_noise() {
        let a = encode(&msg(0x0106, 1, 4), BUDGET).concat();
        let b = encode(&msg(0x0204, 0xFF, 130), BUDGET).concat();
        let mut stream = vec![0x13, 0x37];
        stream.extend(&a);
        stream.extend(&b);
        let mut r = Reassembler::default();
        let mut got = Vec::new();
        for c in stream.chunks(5) { got.extend(r.push(c)); }
        assert_eq!(got, [msg(0x0106, 1, 4), msg(0x0204, 0xFF, 130)]);
        assert_eq!(r.discarded, [0x13, 0x37]);
    }

    #[test]
    fn bad_input_is_dropped() {
        let mut r = Reassembler::default();
        // A middle slice with no first slice, a bad Len, a PayLen past the data.
        assert!(r.push(&[0xAA, 6, 2, 0, 1, 1, 2, 3]).is_empty());
        assert!(r.push(&[0xAA, 0]).is_empty());
        let short = r.push(&[0xAA, 9, 0, 0, 0x06, 0x81, 1, 9, 0, 7, 7]);
        assert_eq!(short, [Message { cmd: 0x8106, seq: 1, payload: vec![7, 7] }]);
    }
}

#[test]
fn requested_realme_buds_7_names_resolve_to_air7_profiles() {
    let air7 = crate::types::find_model(None, Some("realme Buds 7")).expect("Buds 7 alias");
    let air7_pro = crate::types::find_model(None, Some("realme Buds 7 Pro")).expect("Buds 7 Pro alias");
    assert_eq!(air7["id"].as_str(), Some("064812"));
    assert_eq!(air7_pro["id"].as_str(), Some("064C12"));
    assert!(crate::types::is_known_name("REALME BUDS 7"));
    assert!(crate::types::is_known_name("realme buds 7 pro"));
}

mod msg {
    use buds_core::msg::*;
    use buds_core::wire::Message;
    use crate::newproto::to_event;
    use super::old_all as old;

    fn m(cmd: u16, seq: u8, payload: &[u8]) -> Message { Message { cmd, seq, payload: payload.to_vec() } }
    fn push_msg(payload: &[u8]) -> Message { m(PUSH, 0xFF, payload) }

    #[test]
    fn bitmap_matches_the_shipping_table() {
        // Buds 4 capture, Enco Buds2, and every single bit.
        let mut maps: Vec<Vec<u8>> = vec![vec![0, 0xFF, 0x77, 0x5A, 0xEA, 0x67, 0x0E, 0x20, 0x07], vec![0, 0x9B, 0x2C, 0x50, 0x80]];
        for bit in 0..72 { let mut v = vec![0u8; 10]; v[1 + bit / 8] = 1 << (bit % 8); maps.push(v); }
        for map in maps {
            let (new, old) = (Bitmap::parse(&map).unwrap(), old::Caps::parse(&map).unwrap());
            for cmd in 0..=0x0FFFu16 {
                assert_eq!(new.allows(cmd), old.supports(cmd), "{map:02X?} cmd {cmd:04X}");
            }
        }
        assert!(Bitmap::default().allows(0x0422) && !Bitmap::default().known());
        assert!(Bitmap::parse(&[0, 0xFF, 0x77, 0x5A, 0xEA, 0x67, 0x0E, 0x20, 0x07]).unwrap().allows(0x012F));
        assert!(Bitmap::parse(&[1, 0xFF]).is_none() && Bitmap::parse(&[0]).is_none());
    }

    #[test]
    fn replies_from_the_buds4_capture() {
        assert_eq!(decode(&m(0x8103, 1, &[0, 0x14, 0x54, 0x06])), Some(Inbound::ProductId(0x065414)));
        assert_eq!(decode(&m(0x8103, 1, &[1, 0, 0, 0])), None);
        assert_eq!(decode(&m(0x8200, 1, &[0, 9, 1, 2, 3, 4, 8, 0x0B, 0xF1, 0xF2, 0xF3])), Some(Inbound::Offered(vec![1, 2, 3, 4, 8, 0x0B, 0xF1, 0xF2, 0xF3])));
        assert_eq!(decode(&m(0x8200, 1, &[0, 2, 2, 1])), Some(Inbound::Offered(vec![2, 1])));
        let fw = [&[0u8, 4][..], b"1,2,138,2,2,138,3,1,01,3,2,105"].concat();
        let Some(Inbound::Firmware(f)) = decode(&m(0x8105, 1, &fw)) else { panic!() };
        let joined: Vec<&str> = f.iter().filter(|x| x.1 == 2).map(|x| x.2.as_str()).collect();
        assert_eq!(joined.join("."), old::firmware_version("1,2,138,2,2,138,3,1,01,3,2,105").unwrap());
        assert_eq!(decode(&m(0x8205, 3, &[1, 2, 1, 0, 2, 0])), Some(Inbound::Registered));
    }

    #[test]
    fn batch_matches_the_shipping_split() {
        let queries = vec![(0x0106, vec![]), (0x010C, vec![1, 1]), (0x010F, vec![])];
        let replies = [(0x0106u16, vec![0, 1, 1, 0x5A]), (0x010C, vec![0, 1, 1, 8, 0]), (0x010F, vec![0, 2])];
        let mut pl = vec![0, replies.len() as u8];
        for (c, p) in &replies { pl.extend_from_slice(&c.to_le_bytes()); pl.extend_from_slice(&(p.len() as u16).to_le_bytes()); pl.extend_from_slice(p); }
        let new = unbatch(&m(0x812F, 7, &pl));
        let packet = old::build_packet(0x812F, 7, &pl);
        let old_parts = old::unbatch(&packet);
        assert_eq!(new.len(), 3);
        for (n, o) in new.iter().zip(&old_parts) {
            assert_eq!((n.cmd, n.seq, &n.payload[..]), (old::cmd_of(o), old::seq_of(o), old::payload_of(o)));
        }
        // A truncated answer ends the list.
        assert_eq!(unbatch(&m(0x812F, 7, &pl[..pl.len() - 1])).len(), 2);
        let _ = queries;
    }

    #[test]
    fn pushes_match_the_shipping_decoder() {
        // (payload, what the shipping decoder makes of it)
        let wear_old = |p: &[u8]| match old::decode(&old::build_packet(PUSH, 0xFF, p)) { Some(old::Event::Wear(w)) => w, _ => panic!() };
        let p = [2, 3, 1, 7, 2, 7, 3, 4];
        assert_eq!(decode(&push_msg(&p)), Some(Inbound::Wear(wear_old(&p))));
        let p = [2, 2, 1, 0x0F, 2, 4];
        assert_eq!(decode(&push_msg(&p)), Some(Inbound::Wear(wear_old(&p))));
        // Battery: level 0 left out, charging bit, same as the shipping decoder.
        let p = [1, 3, 1, 0x64, 2, 0xE4, 3, 0];
        let Some(old::Event::Battery(b)) = old::decode(&old::build_packet(PUSH, 0xFF, &p)) else { panic!() };
        let Some(Inbound::Battery(n)) = decode(&push_msg(&p)) else { panic!() };
        assert_eq!(n.iter().map(|c| (c.part, c.level, c.charging)).collect::<Vec<_>>(), b);
        // ANC mode, smart level.
        assert_eq!(decode(&push_msg(&[3, 1, 1, 0x08, 0x00])), Some(Inbound::Anc(Anc::Mode(8))));
        assert_eq!(decode(&push_msg(&[3, 4, 1, 0x20, 0])), Some(Inbound::Anc(Anc::Smart(0x20))));
        assert!(matches!(old::decode(&old::build_packet(PUSH, 1, &[3, 4, 1, 0x20, 0])), Some(old::Event::SmartLevel(0x20))));
        // The hold cycle push: old drops it, new names it.
        assert_eq!(decode(&push_msg(&[3, 2, 1, 7])), Some(Inbound::Anc(Anc::HoldCycle { kind: 1, mask: 7 })));
        assert!(old::decode(&old::build_packet(PUSH, 1, &[3, 2, 1, 7])).is_none());
        // Fit test: a missing bud is `None`, the shipping decoder says 0xFF.
        assert_eq!(decode(&push_msg(&[4, 1, 1, 2, 1])), Some(Inbound::FitTest { left: Some(1), right: Some(1) }));
        assert_eq!(decode(&push_msg(&[4, 2, 6])), Some(Inbound::FitTest { left: None, right: Some(6) }));
        assert!(matches!(old::decode(&old::build_packet(PUSH, 1, &[4, 2, 6])), Some(old::Event::FitResult(0xFF, 6))));
        // Game mode: the same two byte positions as `game_on`.
        assert_eq!(decode(&push_msg(&[5, 1])), Some(Inbound::GameMode { at1: Some(1), at3: None }));
        assert_eq!(decode(&push_msg(&[5, 0, 0, 1])), Some(Inbound::GameMode { at1: Some(0), at3: Some(1) }));
        assert_eq!(old::game_on(&[5, 0, 0, 1], Some("062410")), Some(true));
        assert_eq!(decode(&push_msg(&[0x0B, 2])), Some(Inbound::PersonalAnc(2)));
        assert_eq!(decode(&push_msg(&[0xF5, 1])), Some(Inbound::HeadGesture(1)));
        assert_eq!(decode(&push_msg(&[0x08, 2, 5])), Some(Inbound::Hearing { kind: 2, status: 5 }));
        assert_eq!(decode(&push_msg(&[0x0F, 0x10, 0, 4])), Some(Inbound::Public(Public::TapLevel(4))));
        assert!(matches!(old::decode(&old::build_packet(PUSH, 1, &[0x0F, 0x10, 0, 4])), Some(old::Event::TapPush(4))));
    }

    #[test]
    fn dual_devices_match() {
        // `<count> <MAC reversed> <len> <state> <flags> <nameLen> <name>`
        let mut p = vec![2];
        for (mac, st, fl, name) in [([6u8, 5, 4, 3, 2, 1], 2u8, 1u8, "Phone"), ([12, 11, 10, 9, 8, 7], 0, 0, "PC")] {
            p.extend_from_slice(&mac);
            p.extend_from_slice(&[3 + name.len() as u8, st, fl, name.len() as u8]);
            p.extend_from_slice(name.as_bytes());
        }
        let Some(Inbound::Devices(n)) = decode(&push_msg(&[&[6u8][..], &p].concat())) else { panic!() };
        let o = old::parse_devices(&p).unwrap();
        assert_eq!(n.len(), o.len());
        for (a, b) in n.iter().zip(&o) {
            assert_eq!((a.mac, &a.name, a.connected, a.this_phone()), (b.mac, &b.name, b.connected, b.this_device));
        }
        assert_eq!(n[0].mac, [1, 2, 3, 4, 5, 6]);
        assert_eq!(decode(&push_msg(&[6, 1, 1, 2, 3])), None);
    }

    #[test]
    fn ear_scan_matches() {
        let data = vec![9u8; 168];
        let p = [&[0x0Eu8, 3][..], &168u16.to_le_bytes(), &data, &0x01020304u32.to_be_bytes()].concat();
        assert_eq!(decode(&push_msg(&p)), Some(Inbound::EarScan { id: 0x01020304, data: data.clone() }));
        let Some(old::Event::Hearing(old::HearingEv::EarScan(id, d))) = old::decode(&old::build_packet(PUSH, 1, &p)) else { panic!() };
        assert_eq!((id, d), (0x01020304, data));
        assert_eq!(decode(&push_msg(&[0x0E, 3, 0, 0])), None);
    }

    #[test]
    fn pushes_the_shipping_decoder_ignores() {
        // Gesture, from the capture in PROTOCOL.md section 15: right hold.
        assert_eq!(decode(&push_msg(&[0xF1, 2, 1, 4, 8, 3])), Some(Inbound::Gesture { side: 2, button: 1, act: 4, func: 8, context: 3, options: vec![] }));
        assert_eq!(decode(&push_msg(&[0xF1, 1, 1])), None);
        // Connected devices: one entry.
        let p = [&[0xF2u8, 1][..], &[6, 5, 4, 3, 2, 1], &[3, 0], &[2], b"PC"].concat();
        assert_eq!(decode(&push_msg(&p)), Some(Inbound::Linked(vec![Linked { mac: [1, 2, 3, 4, 5, 6], times: 3, name: "PC".into() }])));
        assert_eq!(decode(&push_msg(b"\xF4{\"cmd\":\"x\"}")), Some(Inbound::Diagnostic("{\"cmd\":\"x\"}".into())));
        assert_eq!(decode(&push_msg(&[0x0A, 1])), Some(Inbound::ZenState(1)));
        assert_eq!(decode(&push_msg(&[0xF6, 2])), Some(Inbound::HeadTry(2)));
        assert_eq!(decode(&push_msg(&[0xF7, 0x84])), Some(Inbound::VoiceAssist { kind: Some(2), flag: true }));
        assert_eq!(decode(&push_msg(&[0xF7, 0x00])), Some(Inbound::VoiceAssist { kind: None, flag: false }));
        assert_eq!(decode(&push_msg(&[0xF8, 0b101])), Some(Inbound::LinkSwitch { state: 1, owner: 0, reset: 1 }));
        assert_eq!(decode(&push_msg(&[0xF3, 9])), Some(Inbound::Unused(0xF3, vec![9])));
        assert_eq!(decode(&push_msg(&[0x10, 1, 2])), Some(Inbound::Unused(0x10, vec![1, 2])));
    }

    #[test]
    fn public_events() {
        assert_eq!(decode(&push_msg(&[0x0F, 1, 0, 6])), Some(Inbound::Public(Public::AlertVolume(6))));
        // Flag bit 2, auto (`00`), then a chosen handheld (`01` and a reversed MAC).
        assert_eq!(decode(&push_msg(&[0x0F, 4, 0, 0])), Some(Inbound::Public(Public::Handheld { auto: true, mac: None })));
        assert_eq!(decode(&push_msg(&[0x0F, 4, 0, 1, 6, 5, 4, 3, 2, 1])), Some(Inbound::Public(Public::Handheld { auto: false, mac: Some([1, 2, 3, 4, 5, 6]) })));
        assert_eq!(decode(&push_msg(&[0x0F, 4, 0, 1, 6, 5])), None);
        assert_eq!(decode(&push_msg(&[0x0F, 1, 0])), None);
    }

    #[test]
    fn triangle_fields_follow_the_flags() {
        let t = |p: &[u8]| match decode(&push_msg(&[&[0x0Du8][..], p].concat())) { Some(Inbound::Triangle(t)) => t, x => panic!("{x:?}") };
        let full = t(&[0x1F, 0, 2, 1, 7, 0x03, 2, b'P', b'C']);
        assert_eq!(full, Triangle { capability: Some(2), version: Some(1), wear: Some(7), status: Some(3), related_name: Some("PC".into()) });
        assert_eq!(t(&[0x04, 0, 5]), Triangle { wear: Some(5), ..Default::default() });
        // Flags set, bytes missing: what is present stays.
        assert_eq!(t(&[0x03, 0, 2]), Triangle { capability: Some(2), ..Default::default() });
    }
    #[test]
    fn battery_and_wear_replies() {
        let Some(Inbound::Battery(c)) = decode(&m(0x8106, 1, &[0, 3, 1, 0x64, 2, 0x64, 3, 0x50])) else { panic!() };
        assert_eq!(c.iter().map(|c| (c.part, c.level)).collect::<Vec<_>>(), [(1, 100), (2, 100), (3, 80)]);
        // Lid closed: only the bud outside answers.
        let Some(Inbound::Battery(c)) = decode(&m(0x8106, 1, &[0, 1, 1, 0x5A])) else { panic!() };
        assert_eq!(c, [Cell { part: 1, level: 90, charging: false }]);
        assert_eq!(decode(&m(0x8106, 1, &[1, 1, 1, 0x5A])), None);
        assert_eq!(decode(&push_msg(&[1, 0])), None);
        // Wearing: with and without a leading status byte.
        assert_eq!(decode(&m(0x8109, 1, &[3, 1, 7, 2, 7, 3, 4])), Some(Inbound::Wear(vec![(1, 7), (2, 7), (3, 4)])));
        assert_eq!(decode(&m(0x8109, 1, &[0, 3, 1, 3, 2, 3, 3, 4])), Some(Inbound::Wear(vec![(1, 3), (2, 3), (3, 4)])));
        assert_eq!(decode(&m(0x8109, 1, &[3, 9, 7, 2, 7, 3, 4])), None);
        // Same bytes through the shipping decoder.
        for p in [&[3u8, 1, 7, 2, 7, 3, 4][..], &[0, 3, 1, 3, 2, 3, 3, 4]] {
            let Some(old::Event::Wear(o)) = old::decode(&old::build_packet(0x8109, 1, p)) else { panic!() };
            assert_eq!(decode(&m(0x8109, 1, p)), Some(Inbound::Wear(o)));
        }
    }

    #[test]
    fn anc_replies_and_pushes() {
        assert_eq!(decode(&m(0x810C, 1, &[0, 1, 1, 0x08, 0x00])), Some(Inbound::Anc(Anc::Mode(8))));
        assert_eq!(decode(&m(0x810C, 1, &[0, 1, 1, 0x00, 0x08])), Some(Inbound::Anc(Anc::Mode(0x0800))));
        assert_eq!(decode(&m(0x810C, 1, &[0, 4, 1, 0x40])), Some(Inbound::Anc(Anc::Smart(0x40))));
        assert_eq!(decode(&m(0x810C, 1, &[0, 2, 1, 0x07, 0x08])), Some(Inbound::Anc(Anc::HoldCycle { kind: 1, mask: 0x0807 })));
        assert_eq!(decode(&m(0x810C, 1, &[0, 1, 2, 3])), Some(Inbound::Anc(Anc::Level(3))));
        assert_eq!(decode(&m(0x810C, 1, &[0xE, 1, 1, 8, 0])), None);
        for p in [&[0u8, 1, 1, 0x80, 0][..], &[0, 4, 1, 0x20], &[0, 2, 3, 1]] {
            let o = match old::decode(&old::build_packet(0x810C, 1, p)) {
                Some(old::Event::AncRaw(v)) => (0, 0, v),
                Some(old::Event::SmartLevel(v)) => (1, 0, v),
                Some(old::Event::HoldMask(k, v)) => (2, k, v),
                x => panic!("{p:02X?} {}", x.is_some()),
            };
            let n = match decode(&m(0x810C, 1, p)) {
                Some(Inbound::Anc(Anc::Mode(v))) => (0, 0, v),
                Some(Inbound::Anc(Anc::Smart(v))) => (1, 0, v),
                Some(Inbound::Anc(Anc::HoldCycle { kind, mask })) => (2, kind, mask),
                x => panic!("{x:?}"),
            };
            assert_eq!(o, n, "{p:02X?}");
        }
        // A write refused with no bud in an ear: status 14.
        assert_eq!(decode(&m(0x8404, 1, &[0x0E])), Some(Inbound::Ack { cmd: 0x0404, status: 14, data: vec![] }));
        assert_eq!(decode(&m(0x8404, 1, &[0])), Some(Inbound::Ack { cmd: 0x0404, status: 0, data: vec![] }));
        assert_eq!(decode(&m(0x8427, 1, &[0, 6])), Some(Inbound::Ack { cmd: 0x0427, status: 0, data: vec![6] }));
    }

    #[test]
    fn gesture_table_from_the_buds4_capture() {
        // `dev=01/btn=01[01:00 02:11 03:00 04:08 06:00 05:07]`, the same for dev 02, and the on-call rows.
        let rows: Vec<[u8; 4]> = [(1u8, 1u8, vec![(1u8, 0u8), (2, 0x11), (3, 0), (4, 8), (6, 0), (5, 7)]), (1, 6, vec![(2, 0), (3, 0), (6, 0)]),
            (2, 1, vec![(1, 0), (2, 0), (3, 0), (4, 8), (6, 0), (5, 7)]), (2, 6, vec![(2, 0), (3, 0), (6, 0)])]
            .into_iter().flat_map(|(d, b, acts)| acts.into_iter().map(move |(a, f)| [d, b, a, f])).collect();
        assert_eq!(rows.len(), 18);
        let payload: Vec<u8> = [0u8, 18].into_iter().chain(rows.iter().flatten().copied()).collect();
        let Some(Inbound::Gestures(g)) = decode(&m(0x8108, 1, &payload)) else { panic!() };
        assert_eq!(g.len(), 18);
        let Some(old::Event::KeyFunctions(o)) = old::decode(&old::build_packet(0x8108, 1, &payload)) else { panic!() };
        assert_eq!(g.iter().map(|b| [b.dev, b.button, b.act, b.func]).collect::<Vec<_>>(), o);
        assert_eq!(decode(&m(0x8108, 1, &[0, 0])), None);
        // The count wins over trailing bytes.
        let Some(Inbound::Gestures(g)) = decode(&m(0x8108, 1, &[0, 1, 1, 1, 1, 0, 9, 9, 9, 9])) else { panic!() };
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn feature_and_eq_replies() {
        let reply = [0, 8, 5, 0, 4, 0, 0x0B, 1, 0x11, 1, 0x18, 1, 6, 0, 0x1B, 0, 0x1D, 1];
        let Some(Inbound::Features(f)) = decode(&m(0x810D, 1, &reply)) else { panic!() };
        assert_eq!(f.len(), 8);
        assert_eq!(f[1], (4, 0));
        assert_eq!(decode(&m(0x810F, 1, &[0, 2])), Some(Inbound::EqCurrent(2)));
        assert_eq!(decode(&m(0x0504, 0xFF, &[1])), Some(Inbound::EqCurrent(1)));
        assert_eq!(decode(&m(0x8124, 1, &[0, 0xFB, 5, 0xFE])), Some(Inbound::BassLevel(-2)));
        let p = crate::types::Preset { id: 4, name: "Rock".into(), freqs: vec![62, 250], gains: vec![3, -1], selected: true, tag: [0xFA, 6] };
        let body = [&[0u8, 1, 1, 0xFA, 6, 4, 4][..], b"Rock", &[2, 62, 0, 3, 0xFA, 0, 0xFF]].concat();
        let Some(Inbound::EqCustom(n)) = decode(&m(0x8122, 1, &body)) else { panic!() };
        let Some(old::Event::EqCustom(o)) = old::decode(&old::build_packet(0x8122, 1, &body)) else { panic!() };
        assert_eq!((n[0].id, n[0].selected, n[0].bands.len()), (o[0].id, o[0].selected, o[0].freqs.len()));
        assert_eq!(n[0].bands, [(62, 3), (250, -1)]);
        let _ = p;
        // A bad preset list is dropped.
        assert_eq!(decode(&m(0x8122, 1, &[0, 2, 0, 0xFA, 6, 1, 1, b'a', 0])), None);
    }
    #[test]
    fn setting_replies_match_the_shipping_decoder() {
        let old_of = |cmd: u16, p: &[u8]| old::decode(&old::build_packet(cmd, 1, p));
        assert_eq!(decode(&m(0x8130, 1, &[0, 6])), Some(Inbound::AlertVolume(6)));
        assert!(matches!(old_of(0x8130, &[0, 6]), Some(old::Event::AlertVolume(6))));
        assert_eq!(decode(&m(0x8130, 1, &[1, 6])), None);
        assert_eq!(decode(&m(0x8133, 1, &[0, 2, 3])), Some(Inbound::TapLevel { level: 2, default: Some(3) }));
        assert_eq!(decode(&m(0x812A, 1, &[0, 2])), Some(Inbound::Spatial(2)));
        assert_eq!(decode(&m(0x0510, 0xFF, &[1])), Some(Inbound::Spatial(1)));
        assert_eq!(decode(&m(0x8134, 1, &[0, 1])), Some(Inbound::HeadGesture(1)));
        // Buds 4 capture: codec LHDC V5, and the mask the table misreads.
        assert_eq!(decode(&m(0x8114, 1, &[0, 8])), Some(Inbound::Codec(8)));
        assert_eq!(decode(&m(0x8123, 1, &[0, 0x46, 0x01])), Some(Inbound::CodecMask(0x0146)));
        assert_eq!(decode(&m(0x812B, 1, &[0, 3, 2, 0, 3])), Some(Inbound::GameSound { selected: 3, offered: vec![0, 3] }));
        assert_eq!(decode(&m(0x811A, 1, &[0, 1])), Some(Inbound::PersonalAncStored(true)));
        assert_eq!(decode(&m(0x8132, 1, &[0, 2, 0])), Some(Inbound::Preferred(None)));
        assert_eq!(decode(&m(0x8132, 1, &[0, 2, 1, 1, 2, 3, 4, 5, 6])), Some(Inbound::Preferred(Some([1, 2, 3, 4, 5, 6]))));
        assert!(matches!(old_of(0x8132, &[0, 2, 1, 1, 2, 3, 4, 5, 6]), Some(old::Event::Preferred(Some([1, 2, 3, 4, 5, 6])))));
        // The device list as a reply.
        let list = [&[1u8][..], &[6, 5, 4, 3, 2, 1], &[8, 2, 1, 5], b"Phone"].concat();
        let Some(Inbound::Devices(d)) = decode(&m(0x8112, 1, &[&[0u8][..], &list].concat())) else { panic!() };
        assert_eq!((d[0].mac, d[0].name.as_str(), d[0].connected), ([1, 2, 3, 4, 5, 6], "Phone", true));
        // The bridge gives the shipping session the events it reads from write replies.
        let ev = |msg: &Message| to_event(msg);
        assert!(matches!(ev(&m(0x8427, 1, &[0, 3])), Some(old::Event::AlertVolume(3))));
        assert!(matches!(ev(&m(0x8412, 1, &[15])), Some(old::Event::PncAck(15))));
        assert!(matches!(ev(&m(0x840E, 1, &[3])), Some(old::Event::Hearing(old::HearingEv::Refused(3)))));
        assert!(ev(&m(0x840D, 1, &[0])).is_none());
    }

    #[test]
    fn golden_sound_replies_match_the_shipping_decoder() {
        let v: [i8; 12] = [-55, -49, -41, -35, -25, -19, -11, -5, 3, 7, 13, 17];
        // `0x0115`: the record the apply frames write, read back.
        let apply = crate::cmd::golden_apply(0x01020304, "2026/09/29 01:53", &v, &[9; 168], 11);
        let rec = &apply[1].1;
        let reply = [&[0u8][..], rec].concat();
        assert_eq!(decode(&m(0x8115, 1, &reply)), Some(Inbound::GoldenRecord { id: 0x01020304, name: "2026/09/29 01:53".into(), values: v }));
        let Some(old::Event::Hearing(old::HearingEv::Active(id, name, vals))) = old::decode(&old::build_packet(0x8115, 1, &reply)) else { panic!() };
        assert_eq!((id, name.as_str(), vals), (0x01020304, "2026/09/29 01:53", v));
        // `0x011E`: `00 03 A8 00 <168> <id>`.
        let scan = [&[0u8, 3][..], &168u16.to_le_bytes(), &[9; 168], &7u32.to_be_bytes()].concat();
        assert_eq!(decode(&m(0x811E, 1, &scan)), Some(Inbound::GoldenScan { id: 7, data: vec![9; 168] }));
        // Filters: 12 floats, six per ear (one biquad of six numbers each side).
        let floats: Vec<u8> = (0..12).flat_map(|i| (i as f32).to_le_bytes()).collect();
        let hf = [&[0u8][..], &9u32.to_be_bytes(), &[4], &12u16.to_le_bytes(), &[0, 2], &floats].concat();
        let Some(Inbound::GoldenFilter { id, enhance, curves }) = decode(&m(0x8116, 1, &hf)) else { panic!() };
        assert_eq!((id, enhance), (9, 2));
        let c = curves.unwrap();
        assert_eq!((c.rate, c.left.len(), c.right.len(), c.right[0]), (44100, 6, 6, 6.0));
        let Some(old::Event::Hearing(old::HearingEv::Filter(oid, oen, Some(oc)))) = old::decode(&old::build_packet(0x8116, 1, &hf)) else { panic!() };
        assert_eq!((oid, oen, oc.0, oc.1, oc.2), (9, 2, 44100, c.left, c.right));
        let sf = [&[0u8][..], &9u32.to_be_bytes(), &[4], &44100u16.to_le_bytes(), &12u16.to_le_bytes(), &[0], &floats].concat();
        let Some(Inbound::GoldenScanFilter { id, curves }) = decode(&m(0x811F, 1, &sf)) else { panic!() };
        assert_eq!((id, curves.rate, curves.left.len()), (9, 44100, 6));
        let Some(old::Event::Hearing(old::HearingEv::ScanCurves(oid, oc))) = old::decode(&old::build_packet(0x811F, 1, &sf)) else { panic!() };
        assert_eq!((oid, oc.0, oc.1), (9, 44100, curves.left));
        assert_eq!(decode(&m(0x811F, 1, &[0, 0, 0, 0, 9, 3, 0, 0, 0, 0, 0])), None);
    }

    #[test]
    fn requests_from_the_buds_are_answered_like_the_shipping_code() {
        let old_answer = |cmd: u16, seq: u8, p: &[u8]| old::answer_for(&old::build_packet(cmd, seq, p), 0x6700_1234);
        let cases: Vec<(u16, Vec<u8>)> = vec![
            (0x0500, vec![0, 0, 0, 0]), (0x0500, vec![]), (0x050C, vec![]), (0x051C, vec![]), (0x051D, vec![]),
            (0x0204, vec![0xF2, 1]),
            (PUSH, [&[0xF4u8][..], br#"{"cmd":"a2dp_caton","ver":"1"}"#].concat()), (PUSH, [&[0xF4u8][..], b"not json"].concat()), (PUSH, vec![0xF4]),
            (PUSH, vec![3, 2, 3, 1, 3]), (0x0501, vec![]), (0x0502, vec![]), (0x0504, vec![1]), (0x0600, vec![]), (0x0900, vec![9]),
            (0x8105, vec![0]), (0x0F00, vec![]),
        ];
        for (cmd, p) in cases {
            let new = answer(&m(cmd, 0x14, &p), 0x6700_1234).map(|a| (a.cmd, a.payload));
            assert_eq!(new, old_answer(cmd, 0x14, &p), "{cmd:04X} {p:02X?}");
        }
        let a = answer(&m(0x0500, 0x14, &[0, 0, 0, 0]), 0x6700_1234).unwrap();
        assert_eq!((a.cmd, a.seq, a.payload), (0x8500, 0x14, vec![0, 0x34, 0x12, 0x00, 0x67]));
        // Nord Buds 3 Pro: the request has a real Seq and a payload; the answer repeats the Seq.
        assert_eq!(answer(&m(0x0500, 0x21, &[0, 0, 0, 0]), 0).unwrap().seq, 0x21);
    }
}

mod cmd {
    use buds_core::cmd::*;
    use buds_core::msg::*;
    use super::old_all as old;

    #[test]
    fn connection_frames_match_the_shipping_builders() {
        assert_eq!(register(&[1, 2, 3]), (0x0205, old::register_payload(&[1, 2, 3])));
        assert_eq!(register(&[1, 2]).1, [2, 1, 2]);
        assert_eq!(register_one(4), (0x0201, vec![4]));
        let all = Bitmap::default();
        assert_eq!(notify_ids(&all, None), old::notify_ids(&old::Caps::default(), None));
        let buds4 = Bitmap::parse(&[0, 0xFF, 0x77, 0x5A, 0xEA, 0x67, 0x0E, 0x20, 0x07]).unwrap();
        let caps4 = old::Caps::parse(&[0, 0xFF, 0x77, 0x5A, 0xEA, 0x67, 0x0E, 0x20, 0x07]).unwrap();
        let offered = [1, 2, 3, 4, 8, 0x0B, 0xF1, 0xF2, 0xF3];
        assert_eq!(notify_ids(&buds4, Some(&offered)), old::notify_ids(&caps4, Some(&offered)));
        // Nord Buds 3 Pro offers battery and wear only (issue #2): nothing else is asked for.
        assert_eq!(notify_ids(&all, Some(&[2, 1])), [1, 2]);
        let queries = vec![(0x0102, vec![0xFF, 0xFF]), (0x0106, vec![]), (0x010C, vec![1, 1])];
        assert_eq!(batch(&queries).1, old::batch_payload(&queries));
    }

    #[test]
    fn product_ids_fold_like_the_shipping_decoder() {
        for id in [0x065414u32, 0x100100, 0x100102, 0x100103, 0x108201, 0x060C12] {
            let b = id.to_le_bytes();
            assert_eq!(Some(product_text(id)), old::product_id(&[0, b[0], b[1], b[2]]), "{id:06X}");
        }
        assert_eq!(product_text(0x065414), "065414");
    }

    #[test]
    fn firmware_joins_the_version_type_2_entries() {
        let parts: Vec<(u8, u8, String)> = vec![(1, 2, "138".into()), (2, 2, "138".into()), (3, 1, "01".into()), (3, 2, "105".into())];
        assert_eq!(firmware_text(&parts).as_deref(), Some("138.138.105"));
        assert_eq!(firmware_text(&[(3, 1, "01".into())]), None);
    }
    #[test]
    fn writes_match_the_shipping_builders() {
        // ANC, from the Buds 4 capture: Off, ANC, Transparency, Deep, Adaptive (bit 11).
        for (bit, bytes) in [(0u8, vec![1, 1, 1]), (1, vec![1, 1, 2]), (2, vec![1, 1, 4]), (4, vec![1, 1, 0x10]), (11, vec![1, 1, 0, 8])] {
            assert_eq!(anc_set(bit), (0x0404, bytes));
            assert_eq!(anc_set(bit).1, old::anc_payload(bit));
        }
        for (kind, mask) in [(1u8, 0x03u32), (1, 0x0807), (3, 0x01), (4, 0x0100), (1, 0x01020304), (1, 0)] {
            assert_eq!(hold_cycle(kind, mask).1, old::hold_modes_payload(mask, kind));
        }
        assert_eq!(hold_cycle(1, 3).1, [2, 1, 3]);
        let table = [Binding { dev: 1, button: 1, act: 2, func: 0x11 }, Binding { dev: 4, button: 6, act: 6, func: 0x1C }];
        assert_eq!(gesture_table(&table).1, old::key_function_payload(&[[1, 1, 2, 0x11], [4, 6, 6, 0x1C]]));
        // Call-control capture: `0401 01 04 06 06 1C`.
        assert_eq!(gesture_table(&table[1..]), (0x0401, vec![1, 4, 6, 6, 0x1C]));
        assert_eq!(feature(0x1D, true), (0x0403, vec![0x1D, 1]));
        assert_eq!(eq_select(2), (0x0406, vec![2]));
        assert_eq!(bass_set(-3, 0), (0x041B, vec![0xFB, 5, 0xFD]));
        // realme dynamic sound: the band byte follows the level.
        assert_eq!(bass_set(4, 2), (0x041B, vec![0xFB, 5, 4, 2]));
        assert_eq!(features(&[5, 4, 0x0B]).1, [3, 5, 4, 0x0B]);
        assert_eq!(anc_read(1, 1), (0x010C, vec![1, 1]));
    }

    #[test]
    fn custom_preset_bytes_match_the_shipping_encoder() {
        let p = EqPreset { id: 5, name: "Mine".into(), selected: false, range: (-6, 6), bands: vec![(62, -2), (250, 0), (1000, 3)] };
        let mut o = old::Preset::new("Mine", vec![62, 250, 1000]);
        o.id = 5;
        o.gains = vec![-2, 0, 3];
        for (a, b) in [(EqAction::Create, old::EQ_CREATE), (EqAction::Save, old::EQ_SAVE), (EqAction::Delete, old::EQ_DELETE)] {
            assert_eq!(eq_preset(a, &p).1, o.encode(b));
        }
        assert_eq!(&eq_preset(EqAction::Save, &p).1[..5], [2, 0xFA, 6, 5, 4]);
    }
    #[test]
    fn settings_match_the_shipping_builders() {
        assert_eq!(codec(8, true).1, old::codec_payload(8, true));
        assert_eq!(codec(2, true).1, old::codec_payload(2, true));
        assert_eq!(codec(3, false).1, [3, 0, 0]);
        for mask in [0x0146u16, 0x00FF, 0x0040, 0x0080, 0] { assert_eq!(codecs_of(mask), old::codecs_of(mask)); }
        assert_eq!(alert_volume(6), (0x0427, vec![6]));
        assert_eq!(tap_level(3), (0x042D, vec![3]));
        assert_eq!(game_sound(3), (0x0423, vec![3, 1]));
        assert_eq!(game_eq(true), (0x0420, vec![1, 1]));
        assert_eq!(spatial(2), (0x0422, vec![2]));
        assert_eq!(spatial_source(0), (0x041E, vec![0]));
        assert_eq!(head_gesture(1), (0x0431, vec![1]));
        assert_eq!(find(true), (0x0400, vec![1]));
        assert_eq!(fit_test(false), (0x0405, vec![0]));
        assert_eq!(personal_anc(2), (0x0412, vec![2]));
        assert_eq!(preferred_read(), (0x0132, vec![2]));
        let mac = [1, 2, 3, 4, 5, 6];
        assert_eq!(device_connect(mac, true).1, [1, 1, 2, 3, 4, 5, 6]);
        assert_eq!(device_connect(mac, false).1[0], 2);
        assert_eq!(preferred_set(None).1, [4, 0]);
        assert_eq!(preferred_set(Some(mac)).1, [4, 1, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn phone_state_flags_follow_the_spec() {
        // Dual connection toggle: `08 00 01` after off, `08 00 00` after on, replayed from the capture.
        assert_eq!(phone_state(PhoneState { auto_switch: Some(true), ..Default::default() }), (0x0413, vec![0x08, 0, 1]));
        assert_eq!(phone_state(PhoneState { auto_switch: Some(false), ..Default::default() }).1, [0x08, 0, 0]);
        let all = PhoneState { version: Some(1), screen_on: Some(true), held: Some(false), auto_switch: Some(true), music: Some(true) };
        assert_eq!(phone_state(all).1, [0x1F, 0, 1, 1, 0, 1, 1]);
        assert_eq!(phone_state(PhoneState::default()).1, [0, 0]);
    }

    #[test]
    fn golden_sound_frames_match_the_shipping_builders() {
        let v: [i8; 12] = [-55, -49, -41, -35, -25, -19, -11, -5, 3, 7, 13, 17];
        assert_eq!(ear_scan(true, 0x01020304), old::ear_scan(true, 0x01020304));
        assert_eq!(hearing_test(false), old::hearing_test(false));
        assert_eq!(tone(2, 3, -30), old::hearing_tone(2, 3, -30));
        assert_eq!(tone_stop(), old::hearing_tone_stop());
        assert_eq!(golden_filter(7, &v), old::hearing_filter(7, &v));
        assert_eq!(golden_scan_filter(7, &[9; 168]), old::hearing_scan_filter(7, &[9; 168]));
        assert_eq!(golden_apply(0x01020304, "2026/09/29 01:53", &v, &[9; 168], 11), old::hearing_apply(0x01020304, "2026/09/29 01:53", &v, &[9; 168], 11));
        assert_eq!(golden_apply(1, "x", &v, &[], 0), old::hearing_apply(1, "x", &v, &[], 0));
        // The capture's apply order: restore `01 01 01 00 0B`, record, scan, switch on.
        assert_eq!(golden_apply(1, "x", &v, &[9; 168], 11).iter().map(|f| f.0).collect::<Vec<_>>(), [0x0411, 0x040E, 0x0415, 0x0403]);
        assert_eq!(golden_apply(1, "x", &v, &[], 0).last(), Some(&(0x0403, vec![0x0B, 1])));
    }

    #[test]
    fn session_senders_match_the_old_constants() {
        // Every frame session.rs builds through core: command and payload as the old inline code sent them.
        assert_eq!((handshake().0, product_id().0, broadcast_codes().0), (old::CMD_HANDSHAKE, old::CMD_QUERY_PRODUCT_ID, old::CMD_QUERY_BROADCAST));
        assert_eq!(register_one(5), (old::CMD_REGISTER_ONE, vec![5]));
        assert_eq!(batch(&[]).0, old::CMD_BATCH);
        assert_eq!(anc_read(2, 3), (old::CMD_QUERY_ANC, vec![2, 3]));
        assert_eq!(feature(0x1D, false), (old::CMD_SET_FEATURE, vec![0x1D, 0]));
        assert_eq!(eq_select(4), (old::CMD_SET_EQ, vec![4]));
        assert_eq!(bass_set(5, 0), (old::CMD_SET_BASSWAVE, vec![0xFB, 5, 5]));
        assert_eq!(find(true).0, old::CMD_FIND_BUDS);
        assert_eq!(fit_test(true).0, old::CMD_FIT_TEST);
        assert_eq!(personal_anc_stored(), (old::CMD_QUERY_PERSONAL_NOISE, vec![]));
        assert_eq!(personal_anc(1).0, old::CMD_PERSONAL_NOISE);
        // Dual connection follow-up: `08 00 <!on>`.
        for on in [true, false] {
            assert_eq!(phone_state(PhoneState { auto_switch: Some(!on), ..Default::default() }), (old::CMD_DUAL_FOLLOWUP, vec![8, 0, !on as u8]));
        }
        assert_eq!((devices_read().0, preferred_read().0), (old::CMD_QUERY_DEVICES, old::CMD_QUERY_PREFERRED));
        assert_eq!(device_connect([0; 6], true).0, old::CMD_MULTI_CONNECT);
        assert_eq!((game_sound(1).0, game_sound_read().0), (old::CMD_GAME_SOUND, old::CMD_QUERY_GAME_SOUND));
        assert_eq!((spatial(1).0, spatial_read().0, spatial_source(0).0), (old::CMD_SET_SPATIAL_TYPE, old::CMD_QUERY_SPATIAL_TYPE, old::CMD_SPATIAL_SOURCE));
        assert_eq!((head_gesture(1).0, head_gesture_read().0), (old::CMD_SET_HEAD_MOTION, old::CMD_QUERY_HEAD_MOTION));
        assert_eq!((alert_volume(3).0, tap_level(3).0, tap_level_read().0), (old::CMD_SET_ALERT_VOLUME, old::CMD_SET_TAP_LEVEL, old::CMD_QUERY_TAP_LEVEL));
        assert_eq!(codec(1, false).0, old::CMD_SET_CODEC);
        // On-call row: one entry for both buds, as the old inline `01 04 <btn> <act> <fn>`.
        assert_eq!(gesture_table(&[Binding { dev: 4, button: 6, act: 6, func: 0x1C }]), (old::CMD_SET_KEY_FUNCTION, vec![1, 4, 6, 6, 0x1C]));
    }
}
