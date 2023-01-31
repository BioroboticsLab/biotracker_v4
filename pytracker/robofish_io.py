import sys
from biotracker import *

import robofish.io
import numpy as np
import matplotlib.pyplot as plt
import sys
import json
import os

import asyncio
from grpclib.client import Channel
from grpclib.server import Server

class TrackRecorder(TrackRecorderBase):
    async def set_config(
        self, component_configuration: "ComponentConfiguration"
    ) -> "Empty":
        return Empty()

    async def load(self, empty: "Empty") -> "Track":
        raise grpclib.GRPCError(grpclib.const.Status.UNIMPLEMENTED)

    async def save(self, track_save_request: "TrackSaveRequest") -> "Empty":
        tracks = track_save_request.tracks
        filename = track_save_request.save_path
        experiment = track_save_request.experiment_state

        self.world_size_cm = (experiment.arena.width_cm,
                              experiment.arena.height_cm)
        self.resolution = (experiment.video_info.width,
                           experiment.video_info.height)
        self.hz = experiment.target_fps
        recorded_entities = {}

        start_frame_number = 2**32
        for track in tracks.values():
            start_frame_number = min(start_frame_number, track.observations[0].frame_number)
        loop.run_in_executor(None, lambda: self.save_tracks(
            filename, start_frame_number, tracks))
        return Empty()

    def save_tracks(self, filename, start_frame_number, tracks):
        path = filename + '.hdf5'
        with robofish.io.File(path, mode='w', world_size_cm=self.world_size_cm, frequency_hz=self.hz) as f:
            for id, track in tracks.items():
                poses = self.track_to_poses(track, start_frame_number)
                f.create_entity(category='organism', name=id, poses=poses)

        if len(tracks) > 0:
            self.plot(f, path)

    def track_to_poses(self, track, min_frame_number):
        poses = []
        center_idx, front_idx = (track.skeleton.center_node_index, track.skeleton.front_node_index)
        assert(front_idx is not None and center_idx is not None)

        last_frame_number = min_frame_number - 1
        nan_pose = [np.nan] * 4
        for entity in track.observations:
            pose = self.feature_to_pose(entity.feature, front_idx, center_idx)
            if entity.frame_number > last_frame_number + 1:
                fill_nan_start = last_frame_number + 1
                fill_nan_end = entity.frame_number
                poses.extend([nan_pose] * (fill_nan_end - fill_nan_start))
            poses.append(pose)
            last_frame_number = entity.frame_number
        return np.array(poses)

    def feature_to_pose(self, feature, front_idx, center_idx):
        assert(len(feature.nodes) >= 2)
        return [*self.pixel_to_world(feature.nodes[center_idx].x,
                                     feature.nodes[center_idx].y), 0, 0]

    def pixel_to_world(self, x, y):
        cm, px = self.world_size_cm, self.resolution
        return [x * cm[0] / px[0] - cm[0] / 2,
                -(y * cm[1] / px[1] - cm[1] / 2)]

    def plot(self, f, filename):
        os.system('robofish-io-evaluate tracks ' + filename)


async def main():
    addr, port = get_address_and_port()
    server = Server([TrackRecorder()])
    await server.start(addr, port)
    await server.wait_closed()

if __name__ == "__main__":
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(main())
