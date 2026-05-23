# Desktop Smoke Checklist

Use this checklist before tagging a desktop alpha or beta release.

## Fresh User Setup

1. Quit Pit2SOP.
2. Move the current local config out of the way:

```bash
mv ~/.pit2sop ~/.pit2sop.backup
```

3. Open `Pit2SOP.app`.
4. Confirm Settings shows missing setup state for saved config, Vault, and DeepSeek key.
5. Choose an existing empty directory as the Vault path.
6. Save Settings.
7. Confirm the Vault directory now contains:

```text
00_Inbox/
01_Pits/
02_SOPs/
99_System/
```

8. Save a DeepSeek API key.
9. Confirm the UI shows `configured` and never echoes the key.
10. Run AI test and confirm it returns available.

## Core Desktop Loop

1. Record a pit from the desktop UI.
2. Confirm a Pit markdown file is created.
3. If a pending patch is generated, apply it.
4. Confirm Pending refreshes and the patch disappears from the pending list.
5. Run Doing with a related task.
6. Confirm Doing returns the generated or updated SOP checklist.
7. Run Search with a keyword from the Pit.
8. Confirm Search returns the Pit or SOP.

## Secret And Restart Behavior

1. Clear the API key.
2. Run AI test and confirm the error is explicit.
3. Quit and reopen `Pit2SOP.app`.
4. Confirm saved Settings persist.
5. Restore the original config if needed:

```bash
rm -rf ~/.pit2sop
mv ~/.pit2sop.backup ~/.pit2sop
```

