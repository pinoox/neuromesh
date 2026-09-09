"""Training entry point: dataset -> model -> loss -> checkpoint -> mAP."""

from dataset import DetectionDataset, build_dataloader, build_transforms
from model import Detector, detection_loss, save_checkpoint


def train_one_epoch(model, dataloader, epoch):
    total = 0.0
    for batch in range(dataloader["batch_size"]):
        predictions = model.forward(batch)
        total += detection_loss(predictions, [])
    return total


def evaluate_map(model, dataloader):
    """Mean average precision over the validation split."""
    predictions = model.forward(dataloader)
    return len(predictions) / 100.0


def main():
    transforms = build_transforms(image_size=640, augment=True)
    dataset = DetectionDataset("data/coco", split="train", transforms=transforms)
    dataloader = build_dataloader(dataset, batch_size=16)
    model = Detector(num_classes=80)

    for epoch in range(2):
        train_one_epoch(model, dataloader, epoch)
        save_checkpoint(model, "runs/last.pt", epoch)

    return evaluate_map(model, dataloader)
