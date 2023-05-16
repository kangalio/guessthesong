document.addEventListener('DOMContentLoaded', () => {
    updateView();

    $("#create-form :input").change(function () {
        updateView();
    });
});

function updateView() {
    const gameMode = document.getElementById('game_mode');
    const playlist = document.getElementById('playlist');
    const explicit = document.getElementById('explicit');

    if (gameMode === null || explicit === null || playlist === null) {
        return;
    }

    if (gameMode.options.length === 1) {
        const group = document.getElementById("game_mode-group");
        if (group)
            group.style.display = 'none';
    }

    if (gameMode.value === 'Themes') {
        document.getElementById('advancedSettingsButton').style.display = 'block';
        document.getElementById('theme-group').style.display = 'block';
    } else {
        document.getElementById('advancedSettingsButton').style.display = 'none';
        document.getElementById('theme-group').style.display = 'none';
        document.getElementById('advancedSettings').className = 'collapse';
    }

    if (nonExplicitThemes.includes(playlist.value))
        explicit.removeAttribute('disabled');
    else
        explicit.setAttribute('disabled', '');
}
