pub async fn download_url(url: &str) -> Vec<u8> {
    tokio::process::Command::new("yt-dlp")
        .arg("-x")
        .args(["-o", "-"])
        .arg(url)
        .output()
        .await
        .unwrap()
        .stdout
}

pub async fn download_best_effort(artists: &[&str], title: &str) -> Vec<u8> {
    /*
    let output = tokio::process::Command::new("yt-dlp")
        .args(["--print", "%(id)s %(title)s"])
        .arg("--flat-playlist")
        .args(["--playlist-end", "5"])
        .args(["--download-sections", "*0-100"]) // 75s plus a few extra secs
        // Yt-dlp doesn't list titles in YTMusic search, so no heuristics, which is too many false
        // results. So we search YT directly. Hopefully the title heuristics will eliminate more
        // false results than not using YTMusic is introducing.
        .arg(format!("https://youtube.com/search?q={} - {}", artists.join(", "), title))
        .output()
        .await
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .lines()
        .map(|line| line.split_once(" ").unwrap())
        .max_by_key(|(_id, yt_title)| {
            let yt_title = yt_title.to_lowercase();

            // Ordering criteria, sorted most-precedence-first
            // https://doc.rust-lang.org/std/cmp/trait.Ord.html#impl-Ord-for-(T,)
            (
                // Title must match
                yt_title.contains(&*title.to_lowercase()),
                // Artist should match (but e.g. OcularNebula doesn't put his name in video titles)
                artists.iter().any(|a| yt_title.contains(&*a.to_lowercase())),
                // Longer title may indicate that this is a remix or special edit or whatever
                std::cmp::Reverse(yt_title.len()),
            )
        })
        .unwrap()
        .0;

    tokio::process::Command::new("yt-dlp")
        .arg("-x")
        .args(["-o", "-"])
        // https://www.reddit.com/r/youtubedl/wiki/howdoidownloadpartsofavideo/
        .args(["--download-sections", "*0-100"]) // 75s plus a few extra secs
        .arg(id)
        .output()
        .await
        .unwrap()
        .stdout
    */

    tokio::process::Command::new("yt-dlp")
        .arg("-x")
        .args(["-o", "-"])
        .args(["--playlist-end", "1"])
        // https://www.reddit.com/r/youtubedl/wiki/howdoidownloadpartsofavideo/
        .args(["--download-sections", "*0-100"]) // 75s plus a few extra secs
        .arg(format!("https://music.youtube.com/search?q={} - {}", artists.join(", "), title))
        .output()
        .await
        .unwrap()
        .stdout
}
