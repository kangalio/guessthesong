# What is this

I scraped https://guessthesong.io's frontend code and reimplemented the backend

# Credit

Big thanks to GuessTheSong's devs for this wonderful online casual multiplayer game

I was allowed to make this repository open-source:

> btw would you say it's ok if i make the github repo public which contains the downloaded .html/.js/.css from guessthesong?

> Yeah that is fine, the CSS, HTML on that site was some of the first front end coding we did though so it is not that great anyway. Maybe when you launch your site you wanna put a credit to the source 🫡

[source](https://discord.com/channels/741670496822886470/741670497304969232/1109044332863946762)

# Changes compared to original

- Custom Spotify/YouTube playlists
- Unicode support
- You can join mid-game
- Round skip and stop game is instant
- Punctuation is ignored when guessing

This is just the list of newly implemented features.
The list of not yet implemented features is much larger.
**Expect bugs and missing features!**

# How to run

1. [Create a Spotify developer application](https://developer.spotify.com/dashboard/create)
1. Find the Client ID and Client Secret values
1. Copy .env.example to .env and overwrite the credentials
1. [Install the Rust programming language](https://www.rust-lang.org/tools/install)
1. Run this Rust project like normal: `cargo run` in the terminal
