// This has to be a seperate file since it has to be loaded at a specific moment in the code. (when the body exists.)
function nightmodeController() {
    if (getCookie('nightmode') === 'true') {
        document.body.setAttribute('data-theme', 'dark'); // set the CSS to darkmode
        document.getElementById('fader').style.background = "#2c2f33";
    } else {
        document.body.removeAttribute('data-theme');
        document.getElementById('fader').style.background = "white";
    }
}

nightmodeController();
