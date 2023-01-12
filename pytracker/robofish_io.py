from biotracker.message_bus import MessageBus
import biotracker.tracking_pb2 as tracking
import biotracker.video_pb2 as video
import robofish.io
import numpy as np
import matplotlib.pyplot as plt
import math
import sys

message_bus = MessageBus()
message_bus.subscribe('ENTITIES')
message_bus.subscribe('SHUTDOWN')
message_bus.subscribe('VIDEO_ENCODER_COMMAND')

hz = 30
world_size_cm = (100, 100)
resolution = (2048, 2048)

def plot(f):
    fig, ax = plt.subplots(1,2, figsize=(10,5))
    f.plot(ax=ax[0])
    f.plot(ax=ax[1], lw_distances=True)
    plt.show()

def pixel_to_world(x, y):
    center_offset = (world_size_cm[0] / 2, world_size_cm[1] / 2)
    return [x * world_size_cm[0] / resolution[0] - center_offset[0],
            y * world_size_cm[1] / resolution[1] - center_offset[1]]

def feature_to_pose(feature):
    front_idx = 0
    middle_idx = 1
    assert(len(feature.nodes) >= 2)
    return [*pixel_to_world(feature.nodes[middle_idx].x,
                          feature.nodes[middle_idx].y), 0, 0]

def add_observations(observed_entities, recorded_entities, sample_count):
    nan_pose = [np.nan] * 4
    for (id, feature) in observed_entities.items():
        pose = feature_to_pose(feature)
        if id not in recorded_entities:
            if sample_count > 0:
                recorded_entities[id] = [nan_pose] * sample_count
            else:
                recorded_entities[id] = []
        recorded_entities[id].append(pose)
    for (id, feature) in recorded_entities.items():
        if id not in observed_entities:
            recorded_entities[id].append(nan_pose)

def record_experiment(message_bus):
    sample_count = 0
    recorded_entities = {}

    with robofish.io.File('test.hdf5', mode='w', world_size_cm=world_size_cm, frequency_hz=hz) as f:
        while True:
            (typ, msg) = message_bus.poll(-1)
            if typ == 'SHUTDOWN':
                sys.exit(0)
            elif typ == 'ENTITIES':
                entities_msg = tracking.Entities.FromString(msg)
                add_observations(entities_msg.entities, recorded_entities, sample_count)
                sample_count += 1
            elif typ == 'VIDEO_ENCODER_COMMAND':
                cmd = video.VideoEncoderCommand.FromString(msg)
                if cmd.state is not None and cmd.state == video.VideoState.STOPPED:
                    break
            else:
                print('Unknown message type: ' + typ.decode())
                break
        for (id, poses) in recorded_entities.items():
            np_poses = np.array(poses)
            print(np_poses)
            f.create_entity(category='organism', name=id, poses=np_poses)
        plot(f)

while True:
    (typ, msg) = message_bus.poll(-1)
    if typ == 'SHUTDOWN':
        sys.exit(0)
    elif typ == 'VIDEO_ENCODER_COMMAND':
        cmd = video.VideoEncoderCommand.FromString(msg)
        if cmd.state is not None and cmd.state == video.VideoState.PLAYING:
            record_experiment(message_bus)
