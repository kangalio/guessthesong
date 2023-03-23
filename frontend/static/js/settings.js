// set when page is loaded switch to on or off based on the cookie
document.addEventListener('DOMContentLoaded', () => {
    //setup a default value in case nothing is there yet
    initVisualizerCookie();


    //toggle the visualizer button to show the correct state
    if (getCookie("visualizer") === 'circle') {
        $('#circle_button').parents('.btn').button('toggle');
    }
    if (getCookie("visualizer") === 'bar') {
        $('#bar_button').parents('.btn').button('toggle');
    }
    if (getCookie("visualizer") === 'random') {
        $('#random_button').parents('.btn').button('toggle');
    }

    if (getCookie('disable_word_censor') === '') {
        $('#checkCensor').bootstrapToggle('on')
    } else {
        $('#checkCensor').bootstrapToggle('off')
    }

    if (getCookie('nightmode') === 'true') {
        $('#checkNightmode').bootstrapToggle('on')
    } else {
        $('#checkNightmode').bootstrapToggle('off')
    }

    // set cookie for the visualizer controller controller
    $("#visualizer-button-group :input").change(function () {
        if (this.id === 'circle_button') {
            setCookie("visualizer", "circle", 365);
        }
        if (this.id === 'bar_button') {
            setCookie("visualizer", "bar", 365);
        }
        if (this.id === 'random_button') {
            setCookie("visualizer", "random", 365);
        }
    });
});


// triggered when the switch is triggered
$(function () {
    $('#checkCensor').change(function () {
        const check = $(this).prop('checked');
        if (check)
            setCookie('disable_word_censor', '1', 0);
        else
            setCookie('disable_word_censor', '1', 900);

    });

    $('#checkNightmode').change(function () {
        const check = $(this).prop('checked');
        if (check) {
            setCookie('nightmode', true, 900);
        } else {
            setCookie('nightmode', false, 900);
        }
        nightmodeController();

    })

});