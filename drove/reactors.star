def reactor(name, agent, model, on, action, filters = [], git = False, after = []):
    """One `eventlog react` runtime in its own herdr tab.

    The Rust runtime owns the lock, resume, intent, voter, veto window,
    violation check and ack; `action` is the script it runs per event
    (.context/bin/*-action.sh). No shell supervisor sits in between.
    """
    slug = name + "-reactor"
    cmd = ["eventlog", "react", "--as", agent, "--on", on, "--timeout", "300s"]
    for f in filters:
        cmd += ["--filter", f]
    if git:
        cmd.append("--git")
    cmd += ["--", "bash", ".context/bin/" + action]
    return herdr.tab("{}: {} reactor".format(model, name), panes = [
        pane(slug,
             serve = cmd,
             ready = output("watching"),
             after = after,
             on_start = ["eventlog", "append", "spawn", "agent=" + agent, "model=" + model,
                         "role=" + slug, "runtime=eventlog-react"],
             on_stop = ["eventlog", "append", "retire", "agent=" + agent, "disposition=stopped"]),
    ])
