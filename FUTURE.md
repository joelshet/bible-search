- add speaking that somehow scrubs really fast and has great speed control (somehow audio plus transcript/timestamp index so that the audio can play almost instantly)
- add offline local storage (indexeddb?) memorization cards concept

## Reading plans

Proposed October 2026. Nothing here is built yet.

The goal is two things: "I'm doing this plan, remember where I am", and "I'm reading straight through, remember which chapters I've marked read".

### Store one fact

Which of the 1,189 chapters have been read, one bit each, indexed by chapter in Bible order. That's 149 bytes, about 200 characters as base64. The KJV's chapters never change, so the index stays valid.

Everything else is worked out from that set.

- Straight through: your place is the first unread chapter in Bible order.
- A plan: a range of books plus a number of days. Your place is the first unread chapter in the range. No position is stored, so the plan and the marks can't disagree.
- A day's portion: split the range into equal shares by verse length, breaking only at chapter boundaries. Chapter counts alone are uneven (Psalm 117 has 2 verses, Psalm 119 has 176). The engine already knows every verse's length.
- A plan is one line of data. "Whole Bible, 365 days" comes to about 3.3 chapters a day. "New Testament, 90 days" and "Psalms, 30 days" are the same shape.

### What you see

- Marking: a quiet "mark read" at the end of each chapter in the reader, which toggles. Marking automatically on scroll would misfire during a fast scroll.
- The map: read chapters get a faint ink wash, separate from the red search hot spots, so the Bible fills in as you go.
- The chapter list beside the text dims read chapters.
- Home: one line, "Next: Exodus 12 to 14", linking to it.
- Choosing a plan: a group of radio buttons in the settings panel.
- No calendar. No start date, no streak, and no "behind". The app only says what's next.

### Keeping the data for ten years

- The read set and the chosen plan go in local storage, next to the reading settings. The app calls `navigator.storage.persist()` to ask the browser to keep them.
- The whole state fits in a link, so it also fits in a QR code on the existing QR screen. Opening the link merges its marks into the ones already there (a chapter read in either place is read).
- That link is the backup, the move from Safari to the installed app, and the phone-to-laptop sync. No server and no account.
- Why it matters: as far as I know, Safari on iPhone can clear a site's storage after seven days without a visit, and an app installed to the home screen starts with its own empty storage. Check both on a real phone before relying on them.

### Not building

Accounts, a sync server, reminders (push notifications need a server), streaks, dates, or a reading history.

### Where this breaks

- A plan that reads a chapter twice. M'Cheyne reads the New Testament and Psalms twice a year. That needs a stored position per plan, which is a different model.
- A plan in a different order, such as chronological, only needs an ordering list added as data.
- A second pass through the Bible: "start again" clears the marks. Export the link first if the old marks matter.

### Build order

1. Marks, the map wash, the "Next" line, and the progress link. This already covers reading straight through.
2. Use it for two weeks. If the map doesn't match what was actually read, explicit marking is the wrong mechanism; change it before adding plans.
3. Plan presets as data.

### Open questions

1. Which plans are wanted. If they're all a range and a pace, step 3 is a few lines of data. M'Cheyne or chronological need more, as above.
2. Whether a plan should know the calendar. The proposal says no. A yes means storing a start date and showing when you're behind.
