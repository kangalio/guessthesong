$('#table').DataTable({
    stateSave: true,
    data: dataFromServer,
    deferRender: true,
    pageLength: 50,
    responsive: {
        details: true
    },
    paging: true,
    select: true,
    colReorder: true,
    dom: 'Blfrtip',

    'lengthMenu': [[10, 25, 50, -1], [10, 25, 50, 'All']],

    columns: [
        {
            data: 'code',
            render: function (data, type, row, meta) {
                return '<a id="' + data.toString() + '"href="/join/' + data.toString() + '" class="btn btn-success" role="button">Join</a>'
            },
            title: 'Join'
        },
        {
            data: 'name',
            render: function (data, type, row, meta) {
                return data
            },
            title: 'Room Name'
        },
        {
            data: 'game_mode',
            render: function (data, type, row, meta) {
                return data
            },
            title: 'Game Mode'
        },
        {
            data: 'theme',
            render: function (data, type, row, meta) {
                return data
            },
            title: 'Theme'
        },
        {
            data: 'players',
            render: function (data, type, row, meta) {
                return data
            },
            title: 'Players'
        },
        {
            data: 'status',
            render: function (data, type, row, meta) {
                if (data.toString() === "Private") {
                    return "<i class=\"fas fa-lock\" style='display: inline-block; width: 100%; color: red;'><div style='display: none;'>1</div></i>"
                } else {
                    return "<i class=\"fas fa-lock-open\" style='display: inline-block; width: 100%; color: green;'><div style='display: none;'>2</div></i>"
                }
            },
            title: 'Status'
        },
        {
            data: 'idle',
            render: function (data, type, row, meta) {
                if (type === "display") {
                    return fancyTimeFormat(data);
                }
                return data;
            },
            title: 'Age'
        },
    ]
});

function is_room_in_array(room_id) {
    for (let room in responsive_state_array) {
        if (responsive_state_array[room].room === room_id) {
            return true
        }
    }
    return false
}

function invert_room_state(room_id) {
    for (let room in responsive_state_array) {
        if (responsive_state_array[room].room === room_id) {
            responsive_state_array[room].is_open = !responsive_state_array[room].is_open;
        }
    }
}

$('#table tbody').on('click', 'tr td', function (e) {
    if ($(e.target).closest('a').length === 0) {
        if (!$(e.target).closest('.btn').not(this).length) {
            let room_id = this.childNodes[0].id;
            if (!is_room_in_array(room_id)) {
                responsive_state_array.push({'room': room_id, 'is_open': true});
            } else {
                invert_room_state(room_id);
            }
        }
    }
});

window.onload = function () {
    if ((Object.keys(dataFromServer).length) === 0) {
        document.getElementById("no-rooms-placeholder").style.display = "block";
    }
};
