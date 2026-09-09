"""Detection dataset and its preprocessing transforms."""


class DetectionDataset:
    """Reads image/annotation pairs and applies the configured transforms."""

    def __init__(self, root, split="train", transforms=None):
        self.root = root
        self.split = split
        self.transforms = transforms
        self.samples = []

    def __len__(self):
        return len(self.samples)

    def __getitem__(self, index):
        image, target = self.samples[index]
        if self.transforms is not None:
            image, target = self.transforms(image, target)
        return image, target


def build_transforms(image_size=640, augment=True):
    """Resize and, for training, augment. Changing this shifts mAP."""
    steps = [("resize", image_size)]
    if augment:
        steps.append(("hflip", 0.5))
    return steps


def build_dataloader(dataset, batch_size=16, shuffle=True):
    return {"dataset": dataset, "batch_size": batch_size, "shuffle": shuffle}
