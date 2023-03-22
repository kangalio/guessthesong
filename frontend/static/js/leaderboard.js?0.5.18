$('#table').DataTable({
    stateSave: true,
    data: dataFromServer,
    deferRender: false,
    pageLength: 50,
    responsive: {
        details: true
    },
    select: true,
    colReorder: true,
    dom: 'Blfrtip',
    columnDefs: [
        {"className": "dt-center", "targets": "_all"}
    ],

    'lengthMenu': [[10, 25, 50, -1], [10, 25, 50, 'All']],

    columns: [
        {
            data: 'username',
            render: function (data, type, row, meta) {
                return meta.row + meta.settings._iDisplayStart + 1;
            },
            title: 'Position'
        },
        {
            data: 'emoji',
            render: function (data, type, row, meta) {
                return '<p class="card-text" id="' + row['username'].toString() + '">😎</p>'
            },
            title: 'Emoji'
        },
        {
            data: 'username',
            render: function (data, type, row, meta) {
                return data
            },
            title: 'Username'
        },
        {
            data: 'num_won',
            render: function (data, type, row, meta) {
                return data
            },
            title: 'Games Won'
        },
        {
            data: 'num_played',
            render: function (data, type, row, meta) {
                return data
            },
            title: 'Games Played'
        },
        {
            data: 'score',
            render: function (data, type, row, meta) {
                return data
            },
            title: 'Total Score <a href="#" data-toggle="tooltip" title="Score is calculated by taking into account the sum total of all games you played. Including how many players there were, the number of rounds, and the amount of time spent!">(?)</a>'
        },
    ],
    order: [[5, 'desc']]
});

$(document).ready(function () {
    $('[data-toggle="tooltip"]').tooltip();
});

document.addEventListener('DOMContentLoaded', () => {
    for (let i = 0; i < dataFromServer.length; i++) {
        updateEmojis(dataFromServer[i]["username"], dataFromServer[i]["emoji"]);
    }

    function updateEmojis(username, emoji) {
        let preview = document.getElementById(username);
        if (emoji >= 0 && emoji < emojiList.length) {
            preview.innerText = emojiList[emoji];
        }
    }

});
