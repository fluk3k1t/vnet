#[cfg(test)]
mod tests {
    use super::*;
    use macaddr::MacAddr6;
    use std::{collections::HashMap, net::Ipv4Addr};
    use vnet::{ArpPacket, IPv4PacketType, handle_arp};

    #[test]
    fn test_handle_arp_request() {
        let my_ip = Ipv4Addr::new(192, 168, 1, 1);
        let my_mac = MacAddr6::new(0, 1, 2, 3, 4, 5);
        let src_ip = Ipv4Addr::new(192, 168, 1, 100);
        let src_mac = MacAddr6::new(10, 11, 12, 13, 14, 15);

        let arp = ArpPacket::mk_request(my_ip, src_ip, src_mac);

        let result = handle_arp(&arp, my_ip, my_mac, HashMap::new(), HashMap::new());

        assert_eq!(result.send_frames.len(), 1);
        assert_eq!(result.updated_arp_cache.get(&src_ip), Some(&src_mac));
    }

    #[test]
    fn test_handle_arp_reply() {
        let my_ip = Ipv4Addr::new(192, 168, 1, 1);
        let my_mac = MacAddr6::new(0, 1, 2, 3, 4, 5);
        let target_ip = Ipv4Addr::new(192, 168, 1, 100);
        let target_mac = MacAddr6::new(10, 11, 12, 13, 14, 15);

        let arp = ArpPacket::mk_reply(my_ip, my_mac, target_ip, target_mac);

        let result = handle_arp(
            &arp,
            my_ip,
            my_mac,
            HashMap::new(),
            HashMap::from([(
                target_ip,
                Vec::from([IPv4PacketType::Debug("dummy".to_string())]),
            )]),
        );

        assert_eq!(result.send_frames.len(), 1);
        assert_eq!(result.updated_arp_cache.get(&target_ip), Some(&target_mac));
    }
}
