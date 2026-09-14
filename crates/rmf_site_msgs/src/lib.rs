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
