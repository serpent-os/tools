// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

pub use stone::header;
pub use stone::payload;

pub use self::read::stream_payloads;

pub mod read {
    use std::{fs::File, path::PathBuf};

    use futures::Stream;
    pub use stone::read::{Error, PayloadKind};
    use stone::Header;
    use tokio::{sync::mpsc, task};
    use tokio_stream::wrappers::ReceiverStream;

    pub async fn stream_payloads(
        path: impl Into<PathBuf>,
    ) -> Result<(Header, impl Stream<Item = Result<PayloadKind, Error>>), Error> {
        // Receive potential error when reading before payloads
        let (setup_sender, mut setup_receiver) = mpsc::channel(1);
        // Receive payloads
        let (payload_sender, payload_receiver) = mpsc::channel(1);

        let path = path.into();

        // Read payloads in blocking context and send them over channel
        task::spawn_blocking(move || {
            let setup = || {
                let file = File::open(path)?;
                stone::read(file)
            };

            match setup() {
                Err(error) => {
                    let _ = setup_sender.blocking_send(Err(error));
                }
                Ok(mut stone) => {
                    let header = stone.header;

                    match stone.payloads() {
                        Err(error) => {
                            let _ = setup_sender.blocking_send(Err(error));
                        }
                        Ok(payloads) => {
                            let _ = setup_sender.blocking_send(Ok(header));

                            for result in payloads {
                                let _ = payload_sender.blocking_send(result);
                            }
                        }
                    }
                }
            }
        });

        match setup_receiver.recv().await.unwrap() {
            // Receive each payload in streaming fashion
            Ok(header) => Ok((header, ReceiverStream::new(payload_receiver))),
            Err(error) => Err(error),
        }
    }
}
