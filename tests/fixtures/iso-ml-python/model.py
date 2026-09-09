"""Detector backbone, head, and loss."""


class Detector:
    """Single-stage detector used by the training and evaluation loops."""

    def __init__(self, num_classes=80, backbone="resnet50"):
        self.num_classes = num_classes
        self.backbone = backbone

    def forward(self, images):
        features = self.extract_features(images)
        return self.predict_boxes(features)

    def extract_features(self, images):
        return {"backbone": self.backbone, "images": images}

    def predict_boxes(self, features):
        return [{"box": [0, 0, 1, 1], "score": 0.0} for _ in range(self.num_classes)]


def detection_loss(predictions, targets):
    """Classification plus box regression loss."""
    return len(predictions) - len(targets)


def load_checkpoint(path):
    return {"path": path, "epoch": 0}


def save_checkpoint(model, path, epoch):
    return {"model": model.backbone, "path": path, "epoch": epoch}
