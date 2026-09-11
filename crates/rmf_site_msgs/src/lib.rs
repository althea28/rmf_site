// Pulls the auto-generated code file from Cargo's temporary directory and wraps the raw structs into namespaces
mod gen {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

pub use gen::*;

pub mod rmf_prototype_msgs {
    pub use crate::gen::rmf_prototype_msgs::*;
    pub mod msg {
        pub use crate::gen::rmf_prototype_msgs::*;
    }
}

pub mod nav_msgs {
    pub use crate::gen::nav_msgs::*;
    pub mod msg {
        pub use crate::gen::nav_msgs::*;
    }
}

pub mod geometry_msgs {
    pub use crate::gen::geometry_msgs::*;
    pub mod msg {
        pub use crate::gen::geometry_msgs::*;
    }
}

pub mod std_msgs {
    pub use crate::gen::std_msgs::*;
    pub mod msg {
        pub use crate::gen::std_msgs::*;
    }
}

pub fn deserialize_uuid_fixed_16<'de, D>(deserializer: D) -> Result<[u8; 16], D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct UuidVisitor;

    impl<'de> serde::de::Visitor<'de> for UuidVisitor {
        type Value = [u8; 16];

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a base64-encoded string or an array of 16 bytes")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(value)
                .map_err(serde::de::Error::custom)?;
            bytes
                .try_into()
                .map_err(|_| serde::de::Error::custom("expected 16 bytes"))
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut arr = [0u8; 16];
            for i in 0..16 {
                arr[i] = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::custom("expected 16 bytes"))?;
            }
            Ok(arr)
        }
    }

    deserializer.deserialize_any(UuidVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_plan() {
        let json_str = r#"{"waypoints": [{"position": [-9.0, 9.0], "arrival_constraints": {"regions": [], "nodes": []}, "progress": 0.0, "maps": [], "departure_blockers": [], "departure_trajectory": [], "departure_action": "", "arrival_action": ""}], "start_time": {"sec": 0, "nanosec": 0}, "plan_id": {"destination_session": {"uuid": "6xoMhGiJTbaBlBAvbboRSg=="}, "plan_version": 0}, "workflow": ""}"#;
        let res: Result<rmf_prototype_msgs::msg::Plan, _> = serde_json::from_str(json_str);
        match res {
            Ok(_) => println!("SUCCESS DESERIALIZING PLAN"),
            Err(e) => panic!("FAILED TO DESERIALIZE PLAN: {e:?}"),
        }
    }
}
