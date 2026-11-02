fn parse_hunk_range(token: &str, sign: char) -> Option<(usize, usize)> {
    let token = token.strip_prefix(sign)?;
    let (start, count) = token.split_once(',').unwrap_or((token, "1"));
    if start.is_empty()
        || count.is_empty()
        || !start.bytes().all(|b| b.is_ascii_digit())
        || !count.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let start = start.parse::<usize>().ok()?;
    let count = count.parse::<usize>().ok()?;
    if start == 0 && count != 0 {
        return None;
    }
    start.checked_add(count)?;
    Some((start, count))
}

// GitHub can truncate a hunk without updating its header. Correct counts within the
// declared bounds, keeping the original positions and content.
pub fn normalized_patch(hunk: &str) -> Option<String> {
    let (header, body) = hunk.split_once('\n')?;
    let mut fields = header.split_whitespace();
    if fields.next()? != "@@" {
        return None;
    }
    let (old, old_max) = parse_hunk_range(fields.next()?, '-')?;
    let (new, new_max) = parse_hunk_range(fields.next()?, '+')?;
    if fields.next()? != "@@" {
        return None;
    }
    let (mut old_count, mut new_count, mut rows) = (0, 0, 0);
    for line in body.lines() {
        match line.as_bytes().first()? {
            b' ' => {
                old_count += 1;
                new_count += 1;
            }
            b'-' => old_count += 1,
            b'+' => new_count += 1,
            b'\\' if line == "\\ No newline at end of file" && rows > 0 => continue,
            _ => return None,
        }
        rows += 1;
        if old_count > old_max || new_count > new_max {
            return None;
        }
    }
    if rows == 0 {
        return None;
    }
    // Use a fixed label so a GitHub path cannot be interpreted as patch syntax.
    Some(format!(
        "--- a/review-snapshot\n+++ b/review-snapshot\n@@ -{old},{old_count} +{new},{new_count} @@\n{body}{}",
        if body.ends_with('\n') { "" } else { "\n" }
    ))
}
